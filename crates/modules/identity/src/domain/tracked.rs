//! The snapshots the module's tracked events carry (ADR 0020), read from its
//! own rows in the transaction of the change, and the plan's enums for its
//! values.
//!
//! Ids, enums, booleans, counts and times only: never an address, a name, a
//! provider subject, a credential id, a scope's name or a redirect URI. A
//! client is named by its id only when Campbooks minted it; a client known
//! by its metadata document has a URL for an id, which is never kept.

use campbooks_kernel::{
    Error, Result,
    tracking::{FIRST_PARTY_CLIENT_IDS, minted_client_id},
};
use chrono::{DateTime, Utc};
use sqlx::PgConnection;
use uuid::Uuid;

use crate::{
    domain::clients::Kind,
    tracking::{
        AccountSnapshot, ClientKind, ClientSnapshot, ConsentSnapshot, LinkedVia, PasskeyAddedVia,
        PasskeySnapshot, Provider, SessionSnapshot, SignInMethod, WayInSnapshot,
    },
    upstream,
};

/// The plan's provider for a provider name (`google`, `microsoft`, `zoho`).
pub fn provider(name: &str) -> Option<Provider> {
    match name {
        "google" => Some(Provider::Google),
        "microsoft" => Some(Provider::Microsoft),
        "zoho" => Some(Provider::Zoho),
        _ => None,
    }
}

/// The plan's provider for a round trip's provider.
pub fn provider_of(provider: upstream::Provider) -> Provider {
    match provider {
        upstream::Provider::Google => Provider::Google,
        upstream::Provider::Microsoft => Provider::Microsoft,
        upstream::Provider::Zoho => Provider::Zoho,
    }
}

/// A provider name the database holds, as the plan's provider.
fn stored_provider(name: &str) -> Result<Provider> {
    provider(name).ok_or_else(|| Error::internal("a stored way in names an unknown provider"))
}

/// The plan's kind of a client.
pub fn client_kind(kind: Kind) -> ClientKind {
    match kind {
        Kind::FirstParty => ClientKind::FirstParty,
        Kind::Workspace => ClientKind::Workspace,
        Kind::Dynamic => ClientKind::Dynamic,
        Kind::MetadataDocument => ClientKind::MetadataDocument,
        Kind::Instance => ClientKind::Instance,
    }
}

/// The kind of the client `id` names, from the shape of the ids Campbooks
/// mints ([`minted_client_id`]); anything else is a metadata-document client
/// (a URL).
pub fn client_kind_of(id: &str) -> ClientKind {
    match minted_client_id(id) {
        Some(id) if FIRST_PARTY_CLIENT_IDS.contains(&id) => ClientKind::FirstParty,
        Some(id) if id.starts_with("cb_") => ClientKind::Workspace,
        Some(id) if id.starts_with(super::instance_apps::CLIENT_ID_PREFIX) => ClientKind::Instance,
        Some(_) => ClientKind::Dynamic,
        None => ClientKind::MetadataDocument,
    }
}

/// Client `id`, when Campbooks minted it (the plan's pattern, and the rule
/// the envelope's `client` actor follows: [`minted_client_id`]), else
/// `None`: a metadata-document client's id is a URL and is never kept.
pub fn client_ref(id: &str) -> Option<String> {
    minted_client_id(id).map(str::to_owned)
}

/// Account `id` as it is now.
pub async fn account(conn: &mut PgConnection, id: Uuid) -> Result<AccountSnapshot> {
    let row: Option<(Uuid, bool, i64, i64, DateTime<Utc>)> = sqlx::query_as(
        "SELECT a.id, a.disabled_at IS NOT NULL,
                (SELECT count(*) FROM identity.provider_identities p WHERE p.account_id = a.id),
                (SELECT count(*) FROM identity.passkeys k WHERE k.account_id = a.id),
                a.created_at
           FROM identity.accounts a WHERE a.id = $1",
    )
    .bind(id)
    .fetch_optional(conn)
    .await?;
    let (id, suspended, provider_ways_in, passkeys, created_at) =
        row.ok_or_else(|| Error::internal("a tracked account is gone"))?;
    Ok(AccountSnapshot {
        id,
        suspended,
        provider_ways_in,
        passkeys,
        created_at,
    })
}

/// Session `id` as it is now.
pub async fn session(conn: &mut PgConnection, id: Uuid) -> Result<SessionSnapshot> {
    #[allow(
        clippy::type_complexity,
        reason = "one row of seven columns, read once"
    )]
    let row: Option<(
        Uuid,
        Uuid,
        String,
        Option<String>,
        DateTime<Utc>,
        DateTime<Utc>,
        Option<DateTime<Utc>>,
    )> = sqlx::query_as(
        "SELECT s.id, s.account_id, s.method, p.provider, s.auth_time, s.expires_at, s.ended_at
           FROM identity.sessions s
           LEFT JOIN identity.provider_identities p ON p.id = s.provider_identity_id
          WHERE s.id = $1",
    )
    .bind(id)
    .fetch_optional(conn)
    .await?;
    let (id, account_id, method, provider_name, auth_time, expires_at, ended_at) =
        row.ok_or_else(|| Error::internal("a tracked session is gone"))?;
    let method = if method == "passkey" {
        SignInMethod::Passkey
    } else {
        SignInMethod::Provider
    };
    Ok(SessionSnapshot {
        id,
        account_id,
        method,
        provider: provider_name.as_deref().and_then(provider),
        auth_time,
        expires_at,
        ended_at,
    })
}

/// Provider way in `id` as it is now.
pub async fn way_in(conn: &mut PgConnection, id: Uuid) -> Result<WayInSnapshot> {
    #[allow(
        clippy::type_complexity,
        reason = "one row of seven columns, read once"
    )]
    let row: Option<(
        Uuid,
        Uuid,
        String,
        String,
        bool,
        DateTime<Utc>,
        Option<DateTime<Utc>>,
    )> = sqlx::query_as(
        "SELECT id, account_id, provider, linked_via, email_verified_at_link, linked_at,
                last_used_at
           FROM identity.provider_identities WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(conn)
    .await?;
    let (id, account_id, provider_name, via, address_verified, linked_at, last_used_at) =
        row.ok_or_else(|| Error::internal("a tracked way in is gone"))?;
    let linked_via = match via.as_str() {
        "sign_up" => LinkedVia::SignUp,
        "link" => LinkedVia::Link,
        _ => LinkedVia::Connect,
    };
    Ok(WayInSnapshot {
        id,
        account_id,
        provider: stored_provider(&provider_name)?,
        linked_via,
        address_verified,
        linked_at,
        last_used_at,
    })
}

/// Passkey `id` as it is now, if it exists.
pub async fn passkey(conn: &mut PgConnection, id: Uuid) -> Result<Option<PasskeySnapshot>> {
    #[allow(
        clippy::type_complexity,
        reason = "one row of seven columns, read once"
    )]
    let row: Option<(
        Uuid,
        Uuid,
        String,
        bool,
        bool,
        DateTime<Utc>,
        Option<DateTime<Utc>>,
    )> = sqlx::query_as(
        "SELECT id, account_id, added_via, backup_eligible, backup_state, created_at,
                last_used_at
           FROM identity.passkeys WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(conn)
    .await?;
    Ok(row.map(
        |(id, account_id, via, backup_eligible, backup_state, created_at, last_used_at)| {
            PasskeySnapshot {
                id,
                account_id,
                added_via: if via == "sign_up" {
                    PasskeyAddedVia::SignUp
                } else {
                    PasskeyAddedVia::Passkey
                },
                backup_eligible,
                backup_state,
                created_at,
                last_used_at,
            }
        },
    ))
}

/// Passkey `id` as it is now; it must exist.
pub async fn existing_passkey(conn: &mut PgConnection, id: Uuid) -> Result<PasskeySnapshot> {
    passkey(conn, id)
        .await?
        .ok_or_else(|| Error::internal("a tracked passkey is gone"))
}

/// Client `client_id` as it is now (the transaction must see it: its
/// workspace, or the authorization server's role).
pub async fn client(conn: &mut PgConnection, client_id: &str) -> Result<ClientSnapshot> {
    #[allow(
        clippy::type_complexity,
        reason = "one row of seven columns, read once"
    )]
    let row: Option<(
        String,
        String,
        bool,
        i64,
        i64,
        DateTime<Utc>,
        Option<DateTime<Utc>>,
    )> = sqlx::query_as(
        "SELECT client_id, kind, auth_method <> 'none',
                cardinality(allowed_scopes)::int8, cardinality(redirect_uris)::int8,
                created_at, secret_rotated_at
           FROM identity.clients WHERE client_id = $1",
    )
    .bind(client_id)
    .fetch_optional(conn)
    .await?;
    let (id, kind, confidential, scope_count, redirect_uri_count, created_at, secret_rotated_at) =
        row.ok_or_else(|| Error::internal("a tracked client is gone"))?;
    let kind = match kind.as_str() {
        "workspace" => ClientKind::Workspace,
        "dynamic" => ClientKind::Dynamic,
        "instance" => ClientKind::Instance,
        _ => ClientKind::MetadataDocument,
    };
    Ok(ClientSnapshot {
        client_id: client_ref(&id),
        kind,
        confidential,
        scope_count,
        redirect_uri_count,
        created_at,
        secret_rotated_at,
    })
}

/// Consent `id` as it is now.
pub async fn consent(conn: &mut PgConnection, id: Uuid) -> Result<ConsentSnapshot> {
    #[allow(
        clippy::type_complexity,
        reason = "one row of eight columns, read once"
    )]
    let row: Option<(
        Uuid,
        Uuid,
        String,
        Option<Uuid>,
        i64,
        bool,
        DateTime<Utc>,
        Option<DateTime<Utc>>,
    )> = sqlx::query_as(
        "SELECT id, account_id, client_id, bound_workspace_id, cardinality(scopes)::int8,
                remembered, granted_at, revoked_at
           FROM identity.consents WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(conn)
    .await?;
    let (
        id,
        account_id,
        client,
        bound_workspace_id,
        scope_count,
        remembered,
        granted_at,
        revoked_at,
    ) = row.ok_or_else(|| Error::internal("a tracked consent is gone"))?;
    Ok(ConsentSnapshot {
        id,
        account_id,
        client_id: client_ref(&client),
        client_kind: client_kind_of(&client),
        bound_workspace_id,
        scope_count,
        remembered,
        granted_at,
        revoked_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_kernels_first_party_ids_are_the_issuers() {
        use crate::domain::clients::{NATIVE_CLIENT_ID, WEB_CLIENT_ID};
        assert_eq!(FIRST_PARTY_CLIENT_IDS, [WEB_CLIENT_ID, NATIVE_CLIENT_ID]);
        assert_eq!(client_kind_of(WEB_CLIENT_ID), ClientKind::FirstParty);
        assert_eq!(client_kind_of(NATIVE_CLIENT_ID), ClientKind::FirstParty);
    }

    #[test]
    fn only_ids_campbooks_minted_are_kept() {
        assert_eq!(
            client_ref("campbooks-web").as_deref(),
            Some("campbooks-web")
        );
        let workspace = "cb_AAAAAAAAAAAAAAAAAAAAAA";
        assert_eq!(client_ref(workspace).as_deref(), Some(workspace));
        assert_eq!(client_kind_of(workspace), ClientKind::Workspace);
        let dynamic = "dcr_abcdefghijklmnopqrst-_";
        assert_eq!(client_ref(dynamic).as_deref(), Some(dynamic));
        assert_eq!(client_kind_of(dynamic), ClientKind::Dynamic);
        let instance = "app_AAAAAAAAAAAAAAAAAAAAAA";
        assert_eq!(client_ref(instance).as_deref(), Some(instance));
        assert_eq!(client_kind_of(instance), ClientKind::Instance);
        let document = "https://app.example.com/oauth/client.json";
        assert_eq!(client_ref(document), None);
        assert_eq!(client_kind_of(document), ClientKind::MetadataDocument);
        assert_eq!(client_ref("cb_short"), None);
        assert_eq!(client_ref("cb_AAAAAAAAAAAAAAAAAAAA@example"), None);
    }
}
