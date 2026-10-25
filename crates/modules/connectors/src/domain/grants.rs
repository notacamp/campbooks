//! Turning a provider grant that is already in hand into a connection: the
//! one seam every way of obtaining a grant goes through. OAuth grants come
//! from the identity module (a sign-up's held grant, or a connect or
//! reconnect through its front door) by way of the kernel's grant-intake
//! port; an IMAP mailbox's password comes from `connect_imap`, once its
//! servers accepted it.

use campbooks_kernel::{
    AppState, Error, Result,
    blobs::{BlobOwner, BlobRef},
    db::BeginFor as _,
    tenancy::{UserId, WorkspaceId},
};
use chrono::{DateTime, Utc};
use secrecy::SecretString;
use sqlx::{Postgres, Transaction};

use super::{
    address_checks,
    connections::{self, parse_capabilities},
    credentials,
    imap_servers::{self, ImapAccount},
    model::{Capability, ConnectionId, ConnectionView, InboxId, Phase, Provider, StreamId},
    removal::CONNECTION_OWNER,
    schedule, tracked,
};
use crate::{
    MESSAGES,
    calendar::{self, API_NOT_ENABLED, LIST_CONTENT_TYPE, ListDocument, ListedCalendar},
    deps::Deps,
    events::{ACCOUNT_CONTENT_TYPE, AccountDocument, CapabilityGranted, ConnectionAdded},
    google::CalendarError,
    provider::TokenGrant,
    tracking,
};

/// A provider grant already obtained for a member.
#[derive(Debug, Clone)]
pub struct ProviderGrant {
    /// The workspace the connection belongs to.
    pub workspace: WorkspaceId,
    /// The member whose grant it is.
    pub user: UserId,
    /// The provider.
    pub provider: Provider,
    /// The provider's stable account id (the `OpenID` `sub`).
    pub account_subject: String,
    /// The account's email address.
    pub address: String,
    /// Whether the provider verified that address; an unverified one is
    /// kept, marked as such, and never treated as the account's identity.
    pub address_verified: bool,
    /// The account's display name.
    pub display_name: Option<String>,
    /// The capabilities granted (and wanted); never empty.
    pub capabilities: Vec<Capability>,
    /// What Campbooks keeps to reach the account: OAuth tokens, or an IMAP
    /// mailbox's password.
    pub secret: Secret,
    /// An IMAP mailbox's servers and username (provider `imap` only).
    pub imap: Option<ImapAccount>,
    /// The connection this consent extends, when it was started as one.
    pub extending: Option<ConnectionId>,
    /// The grantor as the identity module knows them (its token subject),
    /// when the grant came through it; announced with the connection.
    pub grantor_subject: Option<String>,
    /// The accounts server of a Zoho grant's data centre.
    pub provider_server: Option<String>,
    /// The mailboxes the grant reaches (one for Gmail; a Zoho login's mail
    /// accounts). Never empty for `messages`.
    pub mailboxes: Vec<Mailbox>,
}

/// What a grant keeps to reach the account, sealed at rest and never
/// returned.
#[derive(Debug, Clone)]
pub enum Secret {
    /// OAuth tokens: access, refresh (required for a new connection),
    /// scopes.
    Tokens(TokenGrant),
    /// An IMAP mailbox's password, which its servers accepted.
    Password(SecretString),
}

impl Secret {
    /// The scopes granted, space-separated (none for a password).
    fn scope(&self) -> String {
        match self {
            Self::Tokens(tokens) => tokens.scope.clone().unwrap_or_default(),
            Self::Password(_) => String::new(),
        }
    }
}

/// A mailbox a grant reaches: one `messages` stream and inbox.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mailbox {
    /// The provider's key for it (Zoho's `accountId`; empty for Gmail).
    pub key: String,
    /// Its address, lower-cased.
    pub address: String,
    /// Whether the provider vouches for the address.
    pub address_verified: bool,
    /// Its display name.
    pub display_name: Option<String>,
}

/// What [`save`] stores.
struct Pending<'a> {
    grant: &'a ProviderGrant,
    /// Each mailbox with its account document.
    mailboxes: Vec<(&'a Mailbox, BlobRef)>,
    /// The account's calendars, when the grant brings the calendar
    /// capability.
    calendars: Option<CalendarGrant>,
    now: DateTime<Utc>,
}

/// The calendars a calendar grant listed.
struct CalendarGrant {
    /// What the provider listed (nothing when it could not be asked).
    listed: Vec<ListedCalendar>,
    /// The `calendar-list` v1 document of them, a blob.
    blob: BlobRef,
    /// Why the calendar stream cannot work yet, for the member to act on.
    reason: Option<&'static str>,
}

/// Create the connection of `grant`, or refresh the existing one of the
/// same account in the workspace: store its credentials sealed, announce it
/// (`connection_added`, `capability_granted` with the account document),
/// create the streams of new capabilities, resume paused ones, and start
/// syncing. Refuses an account another member connected, or one that does
/// not match the connection being extended.
pub async fn create_or_refresh_connection(
    state: &AppState,
    deps: &Deps,
    grant: &ProviderGrant,
) -> Result<ConnectionView> {
    // Blobs are stored before the transaction that references them.
    let mut mailboxes = Vec::with_capacity(grant.mailboxes.len());
    for mailbox in &grant.mailboxes {
        let document = AccountDocument {
            version: 1,
            provider: grant.provider,
            address: mailbox.address.trim().to_lowercase(),
            address_verified: mailbox.address_verified,
            display_name: mailbox.display_name.clone(),
        };
        let blob = state
            .blobs
            .put(
                &grant.workspace.to_string(),
                serde_json::to_vec(&document).map_err(Error::internal)?,
                ACCOUNT_CONTENT_TYPE,
            )
            .await?;
        mailboxes.push((mailbox, blob));
    }
    let calendars = if grant.capabilities.contains(&Capability::Calendar) {
        Some(list_calendars(state, deps, grant).await?)
    } else {
        None
    };
    let mut tx = state.db.begin_for(grant.workspace).await?;
    let pending = Pending {
        grant,
        mailboxes,
        calendars,
        now: deps.clock.now(),
    };
    let id = save(&mut tx, state, deps, &pending).await?;
    let connection = connections::load(&mut tx, id)
        .await?
        .ok_or(Error::NotFound)?;
    let view = connections::view(&mut tx, connection, Some(grant.user)).await?;
    tx.commit().await?;
    Ok(view)
}

/// The calendars of the account `grant` reaches, listed with the grant's
/// own access token, and their `calendar-list` document stored as a blob
/// (before the transaction that references it). A provider that cannot be
/// asked now (trouble, or its calendar API is not enabled for this
/// instance) lists none, and the stream lists them again when it runs: mail
/// is never held back by it.
async fn list_calendars(
    state: &AppState,
    deps: &Deps,
    grant: &ProviderGrant,
) -> Result<CalendarGrant> {
    let mut reason = None;
    let mut listed = Vec::new();
    if let Secret::Tokens(tokens) = &grant.secret {
        let answer = match grant.provider {
            Provider::Google => match &deps.google {
                Some(google) => {
                    let api = google.calendar(&tokens.access_token);
                    Some(calendar::google::list(&api).await)
                }
                None => None,
            },
            Provider::Zoho => {
                let centre = deps
                    .zoho
                    .as_ref()
                    .zip(grant.provider_server.as_deref())
                    .and_then(|(zoho, server)| zoho.centre(server).map(|c| (zoho, c)));
                match centre {
                    Some((zoho, centre)) if deps.config.zoho_calendar => {
                        let api = zoho.calendar(&centre, &tokens.access_token);
                        Some(calendar::zoho::list(&api).await)
                    }
                    _ => None,
                }
            }
            Provider::Imap => None,
        };
        match answer {
            Some(Ok(calendars)) => listed = calendars,
            Some(Err(CalendarError::ApiNotEnabled)) => reason = Some(API_NOT_ENABLED),
            Some(Err(_)) | None => {}
        }
    }
    let document = ListDocument {
        version: 1,
        provider: match grant.provider {
            Provider::Google => Some("google"),
            Provider::Zoho => Some("zoho"),
            Provider::Imap => None,
        },
        calendars: listed.clone(),
    };
    let blob = state
        .blobs
        .put(
            &grant.workspace.to_string(),
            serde_json::to_vec(&document).map_err(Error::internal)?,
            LIST_CONTENT_TYPE,
        )
        .await?;
    Ok(CalendarGrant {
        listed,
        blob,
        reason,
    })
}

/// Create or extend the connection, store its credentials, create the
/// streams of new capabilities and start them. Returns the connection.
#[allow(
    clippy::too_many_lines,
    reason = "one transaction read top to bottom: connection, credentials, streams, events"
)]
async fn save(
    tx: &mut Transaction<'static, Postgres>,
    state: &AppState,
    deps: &Deps,
    g: &Pending<'_>,
) -> Result<ConnectionId> {
    let cipher = deps
        .cipher
        .as_deref()
        .ok_or_else(|| Error::Conflict(MESSAGES.reason("provider_not_configured")))?;
    let workspace_key = g.grant.workspace.to_string();
    let existing = existing(tx, g).await?;
    let had: Vec<Capability> = existing
        .as_ref()
        .map(|(_, had)| had.clone())
        .unwrap_or_default();
    let id = existing
        .as_ref()
        .map_or_else(ConnectionId::new, |(id, _)| *id);
    let mut capabilities = had.clone();
    capabilities.extend_from_slice(&g.grant.capabilities);
    capabilities.sort_unstable();
    capabilities.dedup();
    let capability_names: Vec<String> =
        capabilities.iter().map(|c| c.as_str().to_owned()).collect();
    // How a refreshed connection stood, for the record of the reconnect.
    let before = match &existing {
        Some((id, _)) => {
            let stood = tracked::connection(tx, *id).await?;
            Some((stood.health, stood.mailboxes))
        }
        None => None,
    };
    sqlx::query!(
        r#"INSERT INTO connectors.connections
               (id, workspace_id, provider, account_subject, address, display_name, granted_by,
                capabilities, scopes, health, health_reason, health_changed_at, created_at, updated_at,
                address_verified, provider_server)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, 'ok', NULL, $10, $10, $10, $11, $12)
           ON CONFLICT (id) DO UPDATE
              SET address = $5, display_name = $6, capabilities = $8, scopes = $9,
                  address_verified = $11, provider_server = $12,
                  health = 'ok', health_reason = NULL,
                  health_changed_at = CASE WHEN connectors.connections.health = 'ok'
                                           THEN connectors.connections.health_changed_at ELSE $10 END,
                  updated_at = $10"#,
        id as ConnectionId,
        g.grant.workspace as WorkspaceId,
        g.grant.provider.as_str(),
        g.grant.account_subject,
        g.grant.address,
        g.grant.display_name,
        g.grant.user as UserId,
        &capability_names,
        g.grant.secret.scope(),
        g.now,
        g.grant.address_verified,
        g.grant.provider_server,
    )
    .execute(&mut **tx)
    .await?;
    match &g.grant.secret {
        Secret::Tokens(tokens) => {
            credentials::store(
                tx,
                cipher,
                g.grant.workspace,
                id,
                tokens.refresh_token.as_deref(),
                tokens,
                g.now,
            )
            .await?;
        }
        Secret::Password(password) => {
            credentials::store_password(tx, cipher, g.grant.workspace, id, password, g.now).await?;
        }
    }
    if let Some(account) = &g.grant.imap {
        imap_servers::store(tx, g.grant.workspace, id, account, g.now).await?;
    }
    // An address check proves one address: a connection whose address is
    // not verified (now, perhaps under another address) ends the checks it
    // had under way, whatever the daily budget leaves (campbooks-cloud#314).
    if g.grant.provider == Provider::Imap && !g.grant.address_verified {
        address_checks::abandon(tx, id, g.now).await?;
    }
    for (_, blob) in &g.mailboxes {
        state
            .blobs
            .attach(
                tx,
                &workspace_key,
                blob,
                &BlobOwner::new(CONNECTION_OWNER, id),
            )
            .await?;
    }
    if let Some(calendars) = &g.calendars {
        state
            .blobs
            .attach(
                tx,
                &workspace_key,
                &calendars.blob,
                &BlobOwner::new(CONNECTION_OWNER, id),
            )
            .await?;
    }
    state
        .events
        .publish(
            tx,
            &ConnectionAdded {
                workspace: workspace_key.clone(),
                connection: id,
                provider: g.grant.provider,
                granted_by: g.grant.user,
                account_subject: Some(g.grant.account_subject.clone()),
                grantor_subject: g.grant.grantor_subject.clone(),
                refreshed: existing.is_some(),
            },
        )
        .await?;
    start_streams(tx, state, deps, g, id, &had).await?;
    let connection = tracked::connection(tx, id).await?;
    match before {
        None => {
            state
                .tracker
                .track(
                    tx,
                    &tracking::ConnectionAdded {
                        workspace: g.grant.workspace,
                        connection,
                    },
                )
                .await?;
        }
        Some((before_health, mailboxes)) => {
            let mut added: Vec<Capability> = g
                .grant
                .capabilities
                .iter()
                .copied()
                .filter(|capability| !had.contains(capability))
                .collect();
            added.sort_unstable();
            added.dedup();
            state
                .tracker
                .track(
                    tx,
                    &tracking::ConnectionReconnected {
                        workspace: g.grant.workspace,
                        before_health,
                        capabilities_added: added.into_iter().map(tracked::capability).collect(),
                        mailboxes_added: (connection.mailboxes - mailboxes).max(0),
                        connection,
                    },
                )
                .await?;
        }
    }
    Ok(id)
}

/// The workspace's connection of this account (its id and capabilities),
/// locked, after checking the consent may land on it.
async fn existing(
    tx: &mut Transaction<'static, Postgres>,
    g: &Pending<'_>,
) -> Result<Option<(ConnectionId, Vec<Capability>)>> {
    let existing = sqlx::query!(
        r#"SELECT id AS "id: ConnectionId", granted_by AS "granted_by: UserId", capabilities
             FROM connectors.connections
            WHERE workspace_id = $1 AND provider = $2 AND account_subject = $3
              FOR UPDATE"#,
        g.grant.workspace as WorkspaceId,
        g.grant.provider.as_str(),
        g.grant.account_subject,
    )
    .fetch_optional(&mut **tx)
    .await?;
    if let Some(wanted) = g.grant.extending
        && existing.as_ref().is_none_or(|e| e.id != wanted)
    {
        return Err(Error::Conflict(MESSAGES.reason("account_mismatch")));
    }
    if existing
        .as_ref()
        .is_some_and(|e| e.granted_by != g.grant.user)
    {
        return Err(Error::Conflict(
            MESSAGES.reason("account_connected_by_another_member"),
        ));
    }
    if existing.is_none()
        && matches!(&g.grant.secret, Secret::Tokens(tokens) if tokens.refresh_token.is_none())
    {
        return Err(Error::Denied(MESSAGES.reason("no_offline_access")));
    }
    existing
        .map(|e| Ok((e.id, parse_capabilities(&e.capabilities)?)))
        .transpose()
}

/// Resume a reconnected connection's paused streams, and create, announce
/// and start the streams it did not have: one per new capability, and for
/// `messages` one per mailbox (a refreshed grant can reach a new one).
#[allow(
    clippy::too_many_lines,
    reason = "one transaction read top to bottom: resume, create, announce, start"
)]
async fn start_streams(
    tx: &mut Transaction<'static, Postgres>,
    state: &AppState,
    deps: &Deps,
    g: &Pending<'_>,
    id: ConnectionId,
    had: &[Capability],
) -> Result<()> {
    let workspace_key = g.grant.workspace.to_string();
    // A reconnect resumes the paused streams.
    let resumed = sqlx::query_scalar!(
        r#"UPDATE connectors.streams SET paused = false, throttled_until = NULL,
                  consecutive_failures = 0
            WHERE connection_id = $1
        RETURNING id AS "id: StreamId""#,
        id as ConnectionId
    )
    .fetch_all(&mut **tx)
    .await?;
    for stream in resumed {
        schedule::run_now(tx, g.grant.workspace, stream, g.now).await?;
    }
    // A reconnect gives a calendar stream another chance: whatever kept it
    // from working (the calendar API not enabled) is tried again.
    sqlx::query(
        "UPDATE connectors.streams SET health_reason = NULL
          WHERE connection_id = $1 AND capability = 'calendar'",
    )
    .bind(id)
    .execute(&mut **tx)
    .await?;
    let min_interval = i32::try_from(deps.config.poll.min_seconds).unwrap_or(i32::MAX);
    for &capability in &g.grant.capabilities {
        // `messages` reads each mailbox; other capabilities are one stream
        // per connection, created once.
        let per_mailbox = capability == Capability::Messages;
        if !per_mailbox && had.contains(&capability) {
            continue;
        }
        let targets = if per_mailbox {
            &g.mailboxes[..]
        } else {
            g.mailboxes.get(..1).unwrap_or_default()
        };
        for (mailbox, account) in targets {
            let stream = StreamId::new();
            let inbox = per_mailbox.then(InboxId::new);
            let key = if per_mailbox {
                mailbox.key.as_str()
            } else {
                ""
            };
            let created = sqlx::query_scalar!(
                r#"INSERT INTO connectors.streams
                       (id, workspace_id, connection_id, capability, inbox_id, phase,
                        interval_seconds, created_at, mailbox_key, address)
                   VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
                   ON CONFLICT (connection_id, capability, mailbox_key) DO NOTHING
                   RETURNING id AS "id: StreamId""#,
                stream as StreamId,
                g.grant.workspace as WorkspaceId,
                id as ConnectionId,
                capability.as_str(),
                inbox as Option<InboxId>,
                Phase::Backfill.as_str(),
                min_interval,
                g.now,
                key,
                mailbox.address,
            )
            .fetch_optional(&mut **tx)
            .await?;
            let Some(created) = created else {
                continue;
            };
            let calendars = match (&g.calendars, capability) {
                (Some(grant), Capability::Calendar) => {
                    calendar::remember(tx, g.grant.workspace, created, &grant.listed).await?;
                    if let Some(reason) = grant.reason {
                        sqlx::query(
                            "UPDATE connectors.streams SET health_reason = $2 WHERE id = $1",
                        )
                        .bind(created)
                        .bind(reason)
                        .execute(&mut **tx)
                        .await?;
                    }
                    Some(grant.blob.clone())
                }
                _ => None,
            };
            // The inbox is announced before any batch can name it: the first
            // run cannot commit before this transaction does.
            state
                .events
                .publish(
                    tx,
                    &CapabilityGranted {
                        workspace: workspace_key.clone(),
                        connection: id,
                        capability,
                        granted_by: g.grant.user,
                        inbox_id: inbox,
                        account: account.clone(),
                        calendars,
                    },
                )
                .await?;
            schedule::run_now(tx, g.grant.workspace, stream, g.now).await?;
        }
    }
    Ok(())
}
