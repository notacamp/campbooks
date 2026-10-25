//! `POST /api/v1/w/{workspace_id}/connections/imap` — connect any mailbox
//! with its password (usually an app password). Its servers are found from
//! the address unless the member gives them; Campbooks signs in to the IMAP
//! server (and lists the inbox) and authenticates to the SMTP server
//! (sending nothing) before it keeps anything. Then the connection goes
//! through the seam every grant goes through
//! (`grants::create_or_refresh_connection`): the password sealed like a
//! token, its `messages` stream started, `ConnectionAdded` published.
//!
//! The endpoint makes Campbooks sign in to someone else's server, so it is
//! limited: per member, per workspace, and per target address across every
//! workspace (failed sign-ins only). A server's own words are never
//! repeated.

use axum::{Extension, Json, extract::State, http::StatusCode};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, Result, Risk,
    auth::Principal,
    authz::{self, Action},
    crypto,
    db::BeginFor as _,
    i18n::Locale,
    tenancy::{Access, UserId, Workspace, WorkspaceId},
};
use secrecy::{ExposeSecret as _, SecretString};
use serde::Deserialize;
use utoipa::ToSchema;

use crate::{
    MESSAGES,
    deps::Deps,
    domain::{
        address_checks, connections,
        grants::{Mailbox, ProviderGrant, Secret, create_or_refresh_connection},
        imap_servers::ImapAccount,
        model::{Capability, ConnectionId, ConnectionView, Provider},
        schedule::{self, Rate},
    },
    events::CONNECTION_RECORD,
    features::{
        discover_imap::{admit, spend},
        start_connection::acting_member,
    },
    imap::{
        self, Servers, Source,
        discover::{self, Address, Discovery, Native, PasswordKind, Settings},
    },
    scopes::ConnectorsWrite,
};

/// Connect attempts each member may make per hour.
pub const ATTEMPTS_PER_MEMBER: u32 = 10;
/// Connect attempts per workspace per hour.
pub const ATTEMPTS_PER_WORKSPACE: u32 = 30;
/// Failed sign-ins per target address per hour, across every workspace.
pub const FAILURES_PER_ADDRESS: u32 = 5;

/// The longest password accepted, in bytes.
const MAX_PASSWORD: usize = 1024;
/// The longest username accepted, in characters.
const MAX_USERNAME: usize = 320;

/// A mailbox to connect.
#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ConnectImap {
    /// The mailbox's email address.
    pub address: String,
    /// What it signs in with, when not what the servers are found to want
    /// (usually the address).
    #[serde(default)]
    pub username: Option<String>,
    /// Its password: an app password where the provider has them. Kept
    /// sealed, never shown again.
    #[schema(value_type = String, format = Password)]
    pub password: SecretString,
    /// Its servers, when the member enters them; else they are found from
    /// the address.
    #[serde(default)]
    pub servers: Option<Servers>,
    /// An IMAP connection of yours to connect again (a new password, or
    /// other ports): the same username and IMAP host.
    #[serde(default)]
    pub connection: Option<ConnectionId>,
}

/// Connect a mailbox with its password.
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/connections/imap",
    operation_id = "connectors.connect_imap",
    tag = "connectors",
    security(("bearer" = ["connectors:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low).secret_once().records(["connectors.connection"]).access("connectors.connection", campbooks_kernel::authz::Action::Create)))),
    params(("workspace_id" = WorkspaceId, Path, description = "The workspace.")),
    request_body = ConnectImap,
    responses(
        (status = CREATED, description = "Connected (or connected again): the connection, \
            never its password.", body = ConnectionView),
        (status = BAD_REQUEST, description = "The servers refused or could not be reached \
            (`imap_login_refused`, `smtp_login_refused`, `imap_unreachable`, \
            `smtp_unreachable`, `imap_tls_failed`, `smtp_tls_failed`, \
            `imap_not_an_imap_server`, `host_refused`, `starttls_missing`), no servers were \
            found (`imap_servers_required`), the mailbox is connected natively instead \
            (`use_native_provider`), or the input is not valid (`imap_address_invalid`, \
            `imap_username_invalid`, `imap_password_invalid`).", body = Problem,
            content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, a client token \
            (`connection_needs_user`), or a sign-in more than ten minutes old \
            (`step_up_required`).", body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such workspace, or no such IMAP connection of \
            yours.", body = Problem, content_type = "application/problem+json"),
        (status = CONFLICT, description = "IMAP is not turned on on this instance \
            (`provider_not_configured`), another member connected this mailbox \
            (`account_connected_by_another_member`), or the mailbox is not the one being \
            connected again (`account_mismatch`).", body = Problem,
            content_type = "application/problem+json"),
        (status = TOO_MANY_REQUESTS, description = "Too many attempts by you or this \
            workspace (`rate_limited`), or too many failed sign-ins to this mailbox \
            (`too_many_attempts`).", body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    Extension(deps): Extension<Deps>,
    locale: Locale,
    workspace: Workspace<ConnectorsWrite>,
    Json(input): Json<ConnectImap>,
) -> Result<(StatusCode, Json<ConnectionView>)> {
    let view = execute(
        &state,
        &deps,
        (&workspace.principal, &workspace.access),
        input,
        locale,
    )
    .await?;
    Ok((StatusCode::CREATED, Json(view)))
}

/// Connect `input` for `principal`, the member of `access`; an address the
/// login does not prove is then checked (`address_checks`), its message
/// written in `locale`.
pub async fn execute(
    state: &AppState,
    deps: &Deps,
    (principal, access): (&Principal, &Access),
    input: ConnectImap,
    locale: Locale,
) -> Result<ConnectionView> {
    let user = acting_member(access)?;
    // A connection is owned by the member who makes it (ADR 0017).
    authz::require_type(state, access, Action::Create, CONNECTION_RECORD, || {
        Error::Denied(MESSAGES.reason("connection_needs_user"))
    })?;
    admit(deps, principal)?;
    let address = Address::parse(&input.address)
        .ok_or_else(|| Error::BadRequest(MESSAGES.reason("imap_address_invalid")))?;
    let username = username(input.username.as_deref())?;
    check_password(&input.password)?;
    // An address verified before stays verified when the same mailbox is
    // connected again under it.
    let mut verified_before = false;
    if let Some(connection) = input.connection {
        let mut tx = state.db.begin_for(access.workspace).await?;
        let existing = connections::load(&mut tx, connection).await?;
        tx.commit().await?;
        // Only the grantor connects their own mailbox again.
        let Some(existing) =
            existing.filter(|c| c.granted_by == user && c.provider == Provider::Imap)
        else {
            return Err(Error::NotFound);
        };
        verified_before = existing.address_verified && existing.address == address.address;
    }
    spend(
        state,
        &format!("imap.connect.member:{user}"),
        Rate::per_hour(ATTEMPTS_PER_MEMBER),
    )
    .await?;
    spend(
        state,
        &format!("imap.connect.workspace:{}", access.workspace),
        Rate::per_hour(ATTEMPTS_PER_WORKSPACE),
    )
    .await?;
    let settings = settings(deps, &address, input.servers).await?;
    let username = username.unwrap_or_else(|| settings.username.clone());
    let guard = Failures::take(state, &username, &settings.servers.imap.host).await?;
    let verified = Box::pin(imap::verify(
        &deps.imap,
        &settings.servers,
        &username,
        &input.password,
    ))
    .await;
    if let Err(refusal) = verified {
        tracing::info!(workspace = %access.workspace, %user, code = refusal.code(), "an IMAP mailbox was not connected");
        if refusal.is_login() {
            guard.keep(state).await?;
        } else {
            guard.give_back(state).await?;
        }
        return Err(imap::refusal_error(
            refusal,
            settings.password == PasswordKind::AppPassword,
        ));
    }
    guard.give_back(state).await?;

    let mut grant = ProviderGrant {
        extending: input.connection,
        grantor_subject: Some(principal.subject.clone()),
        ..mailbox_grant(
            access.workspace,
            user,
            address,
            username,
            settings,
            input.password,
        )
    };
    if verified_before {
        grant.address_verified = true;
        for mailbox in &mut grant.mailboxes {
            mailbox.address_verified = true;
        }
    }
    let view = create_or_refresh_connection(state, deps, &grant).await?;
    tracing::info!(workspace = %access.workspace, %user, connection = %view.id, "an IMAP mailbox was connected");
    if view.address_verified {
        return Ok(view);
    }
    check_address(state, deps, access, view.id, locale).await
}

/// The login does not prove connection `id`'s address: check it, with no
/// more of the member than this (campbooks-cloud#314). Past its daily
/// checks, the connection waits for a retry. The connection as it is then.
async fn check_address(
    state: &AppState,
    deps: &Deps,
    access: &Access,
    id: ConnectionId,
    locale: Locale,
) -> Result<ConnectionView> {
    if address_checks::take_budget(&state.db, id).await? {
        let mut tx = state.db.begin_for(access.workspace).await?;
        address_checks::start(&mut tx, access.workspace, id, locale, deps.clock.now()).await?;
        tx.commit().await?;
    }
    let mut tx = state.db.begin_for(access.workspace).await?;
    let connection = connections::load(&mut tx, id)
        .await?
        .ok_or(Error::NotFound)?;
    let view = connections::view(&mut tx, connection, access.user, deps).await?;
    tx.commit().await?;
    Ok(view)
}

/// The grant of `address`'s mailbox, whose servers accepted `username` and
/// `password`: one `messages` stream, a password, never a way to sign in.
fn mailbox_grant(
    workspace: WorkspaceId,
    user: UserId,
    address: Address,
    username: String,
    settings: Settings,
    password: SecretString,
) -> ProviderGrant {
    let servers = settings.servers;
    // A login proves the username, not the address.
    let address_verified = username.eq_ignore_ascii_case(&address.address);
    ProviderGrant {
        workspace,
        user,
        provider: Provider::Imap,
        account_subject: format!(
            "{}@{}",
            username.to_lowercase(),
            servers.imap.host.to_ascii_lowercase()
        ),
        address: address.address.clone(),
        address_verified,
        display_name: None,
        capabilities: vec![Capability::Messages],
        secret: Secret::Password(password),
        imap: Some(ImapAccount {
            servers,
            username,
            source: settings.source,
            provider_hint: settings.provider_hint,
        }),
        extending: None,
        grantor_subject: None,
        provider_server: None,
        mailboxes: vec![Mailbox {
            key: String::new(),
            address: address.address,
            address_verified,
            display_name: None,
        }],
    }
}

/// The username given, trimmed; `None` when none was.
fn username(given: Option<&str>) -> Result<Option<String>> {
    let Some(given) = given.map(str::trim).filter(|u| !u.is_empty()) else {
        return Ok(None);
    };
    if given.chars().count() > MAX_USERNAME || given.chars().any(char::is_control) {
        return Err(Error::BadRequest(MESSAGES.reason("imap_username_invalid")));
    }
    Ok(Some(given.to_owned()))
}

/// A password a server can be given: not empty, not too long, and no line
/// breaks or NUL (which would end the command it is sent in).
fn check_password(password: &SecretString) -> Result<()> {
    let password = password.expose_secret();
    let ok = !password.is_empty()
        && password.len() <= MAX_PASSWORD
        && !password.contains(['\r', '\n', '\0']);
    if ok {
        Ok(())
    } else {
        Err(Error::BadRequest(MESSAGES.reason("imap_password_invalid")))
    }
}

/// The servers to sign in to: the member's, else the ones found for the
/// address. A native provider's mailbox is refused: it is connected with
/// its own sign-in.
async fn settings(deps: &Deps, address: &Address, given: Option<Servers>) -> Result<Settings> {
    let native = |native: Native| {
        Error::BadRequest(
            MESSAGES
                .reason("use_native_provider")
                .arg("provider", native.name()),
        )
    };
    if let Some(native_provider) = discover::native_by_domain(&address.domain) {
        return Err(native(native_provider));
    }
    let settings = match given {
        Some(servers) => {
            let normalized = servers
                .imap
                .normalized()
                .zip(servers.smtp.normalized())
                .ok_or_else(|| Error::BadRequest(MESSAGES.reason("host_refused")))?;
            Settings {
                servers: Servers {
                    imap: normalized.0,
                    smtp: normalized.1,
                },
                username: address.address.clone(),
                source: Source::Manual,
                provider_hint: None,
                password: discover::password_kind(None, &address.domain),
            }
        }
        None => match discover::discover(&deps.imap, address).await {
            Discovery::Settings(settings) => settings,
            Discovery::UseProvider(found) | Discovery::NotSupportedYet(found) => {
                return Err(native(found));
            }
            Discovery::NotFound => {
                return Err(Error::BadRequest(MESSAGES.reason("imap_servers_required")));
            }
        },
    };
    for server in [&settings.servers.imap, &settings.servers.smtp] {
        if let Some(found) = discover::native_by_host(&server.host) {
            return Err(native(found));
        }
    }
    Ok(settings)
}

/// The failed-sign-in budget of one target mailbox (its username on its
/// IMAP server, whatever address was typed), shared by every workspace: an
/// attempt takes one unit before the server is contacted, and gives it back
/// unless the server refused the password.
struct Failures {
    key: String,
}

impl Failures {
    fn rate() -> Rate {
        Rate::per_hour(FAILURES_PER_ADDRESS)
    }

    /// Take a unit for `username` on `imap_host`: 429 `too_many_attempts`
    /// when none is left (the server is not contacted).
    async fn take(state: &AppState, username: &str, imap_host: &str) -> Result<Self> {
        // The mailbox's hash: the key holds no address.
        let mailbox = format!(
            "{}@{}",
            username.to_lowercase(),
            imap_host.to_ascii_lowercase()
        );
        let key = format!(
            "imap.failed:{}",
            URL_SAFE_NO_PAD.encode(crypto::hash(&mailbox))
        );
        match schedule::take(&state.db, &key, 1, Self::rate()).await? {
            Ok(()) => Ok(Self { key }),
            Err(_) => Err(Error::TooManyRequests(MESSAGES.reason("too_many_attempts"))),
        }
    }

    /// The attempt did not fail on the password: the unit goes back.
    async fn give_back(self, state: &AppState) -> Result<()> {
        schedule::settle(&state.db, &self.key, 1, 0, Self::rate()).await
    }

    /// The server refused the password: the unit stays spent. The one that
    /// empties the budget locks the address for about an hour.
    async fn keep(self, state: &AppState) -> Result<()> {
        if schedule::take(&state.db, &self.key, 1, Self::rate())
            .await?
            .is_err()
        {
            schedule::settle(
                &state.db,
                &self.key,
                0,
                FAILURES_PER_ADDRESS - 1,
                Self::rate(),
            )
            .await
        } else {
            schedule::settle(&state.db, &self.key, 1, 0, Self::rate()).await
        }
    }
}

#[cfg(test)]
mod tests;
