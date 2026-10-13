//! The clients that may ask the issuer for tokens.
//!
//! - **First-party** clients are defined in code, not stored, because their
//!   redirect URIs follow `CAMPBOOKS_PUBLIC_URL` and the web client's secret
//!   is derived from the instance secret: a fresh install needs no
//!   provisioning step. `campbooks-web` is the server's own BFF;
//!   `campbooks-native` the desktop and mobile apps (a public client that
//!   always shows the consent screen, RFC 8252 §8.6).
//! - **Stored** clients (`identity.clients`, slice S4): a workspace's own
//!   integrations, clients registered dynamically (RFC 7591) and clients
//!   known by their metadata document. The table is under forced row-level
//!   security; the authorization server reads it in a transaction that says
//!   so ([`begin_as_server`]).
//!
//! Redirect URIs match exactly, except that a loopback redirect
//! (`127.0.0.1`, `[::1]`, `localhost`) matches on any port (RFC 8252 §7.3).

use aws_lc_rs::constant_time;
use campbooks_kernel::{
    Result,
    auth::ScopeInfo,
    crypto::{hash, same_hash},
};
use chrono::{DateTime, TimeDelta, Utc};
use secrecy::{ExposeSecret as _, SecretString};
use sqlx::{PgConnection, PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::protocol::discovery::OIDC_SCOPES;

/// The web app's client: the server's own BFF, a confidential client.
pub const WEB_CLIENT_ID: &str = "campbooks-web";
/// The native apps' client (Tauri): a public client using PKCE.
pub const NATIVE_CLIENT_ID: &str = "campbooks-native";
/// The redirect URI path of the native client on loopback, any port
/// (RFC 8252 §7.3).
pub const NATIVE_LOOPBACK_PATH: &str = "/callback";
/// The native client's private-use URI scheme redirect (RFC 8252 §7.1).
pub const NATIVE_SCHEME_REDIRECT: &str = "com.notacamp.campbooks:/oauth/callback";

/// The prefix of every `identity:*` scope: a person's account, its ways in
/// and the workspace's clients. Only Campbooks' own clients may hold them.
const FIRST_PARTY_ONLY_PREFIX: &str = "identity:";

/// How long the native apps' refresh tokens last unused, and at most.
const NATIVE_REFRESH: (TimeDelta, TimeDelta) = (TimeDelta::days(30), TimeDelta::days(90));
/// The web app's.
const WEB_REFRESH: (TimeDelta, TimeDelta) = (TimeDelta::days(14), TimeDelta::days(30));
/// Every other client's: until consent is revoked, renewed while used.
const THIRD_PARTY_REFRESH: (TimeDelta, TimeDelta) = (TimeDelta::days(30), TimeDelta::days(365));

/// What kind of client it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Campbooks' own (web, native).
    FirstParty,
    /// A workspace's own integration.
    Workspace,
    /// Registered through `/oauth2/register`.
    Dynamic,
    /// Known by its Client ID Metadata Document.
    MetadataDocument,
}

impl Kind {
    /// The wire and database name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FirstParty => "first_party",
            Self::Workspace => "workspace",
            Self::Dynamic => "dynamic",
            Self::MetadataDocument => "metadata_document",
        }
    }

    fn parse(kind: &str) -> Option<Self> {
        match kind {
            "workspace" => Some(Self::Workspace),
            "dynamic" => Some(Self::Dynamic),
            "metadata_document" => Some(Self::MetadataDocument),
            _ => None,
        }
    }
}

/// How a client authenticates at the token endpoint.
pub enum Authentication {
    /// `client_secret_basic` or `client_secret_post`, a secret we can read
    /// (the web client's, derived from the instance secret).
    Secret(SecretString),
    /// The same, against the SHA-256 we keep (a workspace client).
    Hashed(Vec<u8>),
    /// A public client: PKCE only.
    None,
}

/// One client.
pub struct Client {
    /// `client_id`.
    pub id: String,
    /// What kind of client it is.
    pub kind: Kind,
    /// The name shown on the consent screen and in settings.
    pub name: String,
    /// The workspace a workspace client belongs to.
    pub workspace: Option<Uuid>,
    /// A workspace client's logo (the only kind whose logo is shown).
    pub logo_uri: Option<String>,
    /// How it authenticates.
    pub authentication: Authentication,
    /// Redirect URIs it registered.
    pub redirect_uris: Vec<String>,
    /// Grant types it may use.
    pub grant_types: Vec<String>,
    /// The API scopes it may ask for; `None` for first-party clients (every
    /// scope of the catalogue).
    pub allowed_scopes: Option<Vec<String>>,
    /// Where it may send the browser after logout.
    pub post_logout_redirect_uris: Vec<String>,
    /// A refresh token unused this long stops working.
    pub refresh_idle: TimeDelta,
    /// Every refresh token of a sign-in stops working this long after it.
    pub refresh_absolute: TimeDelta,
}

impl std::fmt::Debug for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Client")
            .field("id", &self.id)
            .field("kind", &self.kind)
            .finish_non_exhaustive()
    }
}

impl Client {
    /// Whether `uri` is one of this client's redirect URIs: an exact match,
    /// or a loopback redirect that differs only by its port.
    pub fn accepts_redirect(&self, uri: &str) -> bool {
        if self.redirect_uris.iter().any(|allowed| allowed == uri) {
            return true;
        }
        let Some(asked) = Loopback::parse(uri) else {
            return false;
        };
        self.redirect_uris
            .iter()
            .filter_map(|allowed| Loopback::parse(allowed))
            .any(|allowed| allowed == asked)
    }

    /// Whether the presented secret (or none) authenticates this client.
    pub fn authenticates(&self, presented: Option<&str>) -> bool {
        match (&self.authentication, presented) {
            (Authentication::None, None) => true,
            (Authentication::Secret(secret), Some(presented)) => {
                constant_time::verify_slices_are_equal(
                    secret.expose_secret().as_bytes(),
                    presented.as_bytes(),
                )
                .is_ok()
            }
            (Authentication::Hashed(stored), Some(presented)) => {
                same_hash(stored, &hash(presented))
            }
            _ => false,
        }
    }

    /// Whether it is a confidential client.
    pub fn is_confidential(&self) -> bool {
        !matches!(self.authentication, Authentication::None)
    }

    /// Whether it may use `grant_type`.
    pub fn may_use(&self, grant_type: &str) -> bool {
        self.grant_types.iter().any(|g| g == grant_type)
    }

    /// Whether it is Campbooks' own.
    pub fn is_first_party(&self) -> bool {
        self.kind == Kind::FirstParty
    }

    /// Whether the person must say yes before it gets a code: every client
    /// but the web app.
    pub fn needs_consent(&self) -> bool {
        self.id != WEB_CLIENT_ID
    }

    /// Whether a token for it is bound to one workspace: every client but
    /// Campbooks' own, which have a workspace switcher.
    pub fn binds_workspace(&self) -> bool {
        !self.is_first_party()
    }

    /// Whether the person can trust its name: Campbooks' own, or a client
    /// a workspace admin registered. Anyone can register the others.
    pub fn is_verified(&self) -> bool {
        matches!(self.kind, Kind::FirstParty | Kind::Workspace)
    }

    /// Whether it may ask for `scope`: the OIDC scopes; for Campbooks' own,
    /// every scope of `catalogue`; for anyone else, the ones it registered,
    /// never an `identity:*` scope.
    pub fn may_ask_for(&self, scope: &str, catalogue: &[ScopeInfo]) -> bool {
        if OIDC_SCOPES.contains(&scope) {
            return true;
        }
        if !catalogue.iter().any(|s| s.name == scope) {
            return false;
        }
        match &self.allowed_scopes {
            None => true,
            Some(allowed) => third_party_may_hold(scope) && allowed.iter().any(|s| s == scope),
        }
    }
}

/// Whether a client other than Campbooks' own may ever hold `scope`.
pub fn third_party_may_hold(scope: &str) -> bool {
    !scope.starts_with(FIRST_PARTY_ONLY_PREFIX)
}

/// The scopes of `catalogue` a client other than Campbooks' own may hold.
pub fn third_party_scopes(catalogue: &[ScopeInfo]) -> Vec<String> {
    catalogue
        .iter()
        .filter(|s| third_party_may_hold(s.name))
        .map(|s| s.name.to_owned())
        .collect()
}

/// Whether `uri` is a redirect a code may reach only through whoever
/// controls an `https` URL: a remembered consent may skip the screen only
/// for those. Loopback and private-use schemes can be claimed by any app on
/// the device (RFC 8252 §8.6).
pub fn is_https_redirect(uri: &str) -> bool {
    url::Url::parse(uri).is_ok_and(|url| url.scheme() == "https" && url.host_str().is_some())
}

/// A loopback redirect, without its port.
#[derive(Debug, PartialEq, Eq)]
struct Loopback {
    host: String,
    path: String,
    query: Option<String>,
}

impl Loopback {
    fn parse(uri: &str) -> Option<Self> {
        let url = url::Url::parse(uri).ok()?;
        let loopback = match url.host()? {
            url::Host::Ipv4(ip) => ip.is_loopback(),
            url::Host::Ipv6(ip) => ip.is_loopback(),
            url::Host::Domain(domain) => domain == "localhost",
        };
        (url.scheme() == "http"
            && loopback
            && url.fragment().is_none()
            && url.username().is_empty()
            && url.password().is_none())
        .then(|| Self {
            host: url.host_str().unwrap_or_default().to_owned(),
            path: url.path().to_owned(),
            query: url.query().map(ToOwned::to_owned),
        })
    }
}

/// Whether `uri` is a loopback redirect (`http://127.0.0.1…`, `[::1]`,
/// `localhost`) or a private-use scheme: one an app on the device answers.
pub fn is_device_redirect(uri: &str) -> bool {
    Loopback::parse(uri).is_some()
        || !matches!(url::Url::parse(uri), Ok(u) if u.scheme() == "https")
}

/// Whether a client registering itself (dynamically, or by its metadata
/// document) may use `uri`: `https` with a host, or a loopback `http` URI;
/// never a fragment or userinfo.
pub fn is_registrable_redirect(uri: &str) -> bool {
    if uri.len() > 2048 {
        return false;
    }
    let Ok(url) = url::Url::parse(uri) else {
        return false;
    };
    let clean = url.fragment().is_none() && url.username().is_empty() && url.password().is_none();
    clean && (is_https_redirect(uri) || Loopback::parse(uri).is_some())
}

/// The host a redirect URI sends the browser to, for the consent screen;
/// `None` for a private-use scheme.
pub fn redirect_host(uri: &str) -> Option<String> {
    url::Url::parse(uri)
        .ok()
        .and_then(|url| url.host_str().map(ToOwned::to_owned))
}

/// Every client the issuer knows: the first-party ones here, the rest in
/// the database.
pub struct Clients {
    first_party: Vec<Client>,
}

impl Clients {
    /// The first-party clients of the instance at `origin`; the web client's
    /// secret is `web_secret`.
    pub fn first_party(origin: &str, web_secret: SecretString) -> Self {
        let code = vec!["authorization_code".to_owned(), "refresh_token".to_owned()];
        Self {
            first_party: vec![
                Client {
                    id: WEB_CLIENT_ID.to_owned(),
                    kind: Kind::FirstParty,
                    name: "Campbooks".to_owned(),
                    workspace: None,
                    logo_uri: None,
                    authentication: Authentication::Secret(web_secret),
                    redirect_uris: vec![format!("{origin}/bff/callback")],
                    grant_types: code.clone(),
                    allowed_scopes: None,
                    post_logout_redirect_uris: vec![format!("{origin}/")],
                    refresh_idle: WEB_REFRESH.0,
                    refresh_absolute: WEB_REFRESH.1,
                },
                Client {
                    id: NATIVE_CLIENT_ID.to_owned(),
                    kind: Kind::FirstParty,
                    name: "Campbooks for desktop and mobile".to_owned(),
                    workspace: None,
                    logo_uri: None,
                    authentication: Authentication::None,
                    redirect_uris: vec![
                        format!("http://127.0.0.1{NATIVE_LOOPBACK_PATH}"),
                        format!("http://[::1]{NATIVE_LOOPBACK_PATH}"),
                        NATIVE_SCHEME_REDIRECT.to_owned(),
                    ],
                    grant_types: code,
                    allowed_scopes: None,
                    post_logout_redirect_uris: Vec::new(),
                    refresh_idle: NATIVE_REFRESH.0,
                    refresh_absolute: NATIVE_REFRESH.1,
                },
            ],
        }
    }

    /// The first-party client `id` (RP-initiated logout serves only these).
    pub fn first_party_client(&self, id: &str) -> Option<&Client> {
        self.first_party.iter().find(|client| client.id == id)
    }

    /// The client `id`: first-party, or stored. A metadata-document client
    /// is found here only once [`/oauth2/authorize`](crate::protocol::authorize)
    /// fetched its document.
    pub async fn find(&self, db: &PgPool, id: &str) -> Result<Option<Client>> {
        if let Some(client) = self.first_party_client(id) {
            return Ok(Some(client.duplicate()));
        }
        let mut tx = begin_as_server(db).await?;
        let client = load(&mut tx, id).await?;
        tx.commit().await?;
        Ok(client)
    }
}

impl Client {
    /// A copy (secrets are not `Clone` on purpose; this is the one place).
    fn duplicate(&self) -> Self {
        Self {
            id: self.id.clone(),
            kind: self.kind,
            name: self.name.clone(),
            workspace: self.workspace,
            logo_uri: self.logo_uri.clone(),
            authentication: match &self.authentication {
                Authentication::Secret(secret) => Authentication::Secret(secret.clone()),
                Authentication::Hashed(hash) => Authentication::Hashed(hash.clone()),
                Authentication::None => Authentication::None,
            },
            redirect_uris: self.redirect_uris.clone(),
            grant_types: self.grant_types.clone(),
            allowed_scopes: self.allowed_scopes.clone(),
            post_logout_redirect_uris: self.post_logout_redirect_uris.clone(),
            refresh_idle: self.refresh_idle,
            refresh_absolute: self.refresh_absolute,
        }
    }
}

/// A transaction in which the authorization server may read and write any
/// stored client (`campbooks.identity_role`, local to it).
pub async fn begin_as_server(db: &PgPool) -> Result<Transaction<'static, Postgres>> {
    let mut tx = db.begin().await?;
    as_server(&mut tx).await?;
    Ok(tx)
}

/// Let the rest of the current transaction read and write any stored
/// client.
pub async fn as_server(conn: &mut PgConnection) -> Result<()> {
    sqlx::query("SELECT set_config('campbooks.identity_role', 'authorization_server', true)")
        .execute(conn)
        .await?;
    Ok(())
}

/// A stored client row.
#[derive(sqlx::FromRow)]
pub struct Row {
    /// `client_id`.
    pub client_id: String,
    /// `workspace`, `dynamic` or `metadata_document`.
    pub kind: String,
    /// A workspace client's workspace.
    pub workspace_id: Option<Uuid>,
    /// Name.
    pub name: String,
    /// Logo.
    pub logo_uri: Option<String>,
    /// Redirect URIs.
    pub redirect_uris: Vec<String>,
    /// Grant types.
    pub grant_types: Vec<String>,
    /// SHA-256 of the secret.
    pub secret_hash: Option<Vec<u8>>,
    /// Scopes it may ask for.
    pub allowed_scopes: Vec<String>,
    /// When it was registered.
    pub created_at: DateTime<Utc>,
    /// When its secret last changed.
    pub secret_rotated_at: Option<DateTime<Utc>>,
    /// When it last got a token.
    pub last_used_at: Option<DateTime<Utc>>,
    /// A metadata document's cache expiry.
    pub metadata_expires_at: Option<DateTime<Utc>>,
}

/// The columns of [`Row`].
pub const COLUMNS: &str = "client_id, kind, workspace_id, name, logo_uri, redirect_uris, \
     grant_types, secret_hash, allowed_scopes, created_at, secret_rotated_at, last_used_at, \
     metadata_expires_at";

impl Row {
    /// The client this row describes; `None` for a kind this build does not
    /// know.
    pub fn into_client(self) -> Option<Client> {
        let kind = Kind::parse(&self.kind)?;
        Some(Client {
            id: self.client_id,
            kind,
            name: self.name,
            workspace: self.workspace_id,
            logo_uri: self.logo_uri.filter(|_| kind == Kind::Workspace),
            authentication: match self.secret_hash {
                Some(hash) => Authentication::Hashed(hash),
                None => Authentication::None,
            },
            redirect_uris: self.redirect_uris,
            grant_types: self.grant_types,
            allowed_scopes: Some(self.allowed_scopes),
            post_logout_redirect_uris: Vec::new(),
            refresh_idle: THIRD_PARTY_REFRESH.0,
            refresh_absolute: THIRD_PARTY_REFRESH.1,
        })
    }
}

/// The stored client `id` (the transaction must be [`as_server`]).
pub async fn load(conn: &mut PgConnection, id: &str) -> Result<Option<Client>> {
    Ok(load_row(conn, id).await?.and_then(Row::into_client))
}

/// The stored row of client `id` (the transaction must be [`as_server`]).
pub async fn load_row(conn: &mut PgConnection, id: &str) -> Result<Option<Row>> {
    Ok(sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {COLUMNS} FROM identity.clients WHERE client_id = $1"
    )))
    .bind(id)
    .fetch_optional(conn)
    .await?)
}

/// Record that client `id` got a token, at most once a minute.
pub async fn touch(db: &PgPool, id: &str, now: DateTime<Utc>) -> Result<()> {
    let mut tx = begin_as_server(db).await?;
    sqlx::query(
        "UPDATE identity.clients SET last_used_at = $2
          WHERE client_id = $1 AND (last_used_at IS NULL OR last_used_at < $2 - interval '1 minute')",
    )
    .bind(id)
    .bind(now)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clients() -> Clients {
        Clients::first_party("https://app.example.com", SecretString::from("s3cret"))
    }

    fn public(redirects: &[&str]) -> Client {
        Client {
            id: "dcr_x".to_owned(),
            kind: Kind::Dynamic,
            name: "Agent".to_owned(),
            workspace: None,
            logo_uri: None,
            authentication: Authentication::None,
            redirect_uris: redirects.iter().map(|r| (*r).to_owned()).collect(),
            grant_types: vec!["authorization_code".to_owned()],
            allowed_scopes: Some(vec!["system:read".to_owned(), "identity:read".to_owned()]),
            post_logout_redirect_uris: Vec::new(),
            refresh_idle: THIRD_PARTY_REFRESH.0,
            refresh_absolute: THIRD_PARTY_REFRESH.1,
        }
    }

    #[test]
    fn web_redirects_match_exactly() {
        let clients = clients();
        let web = clients.first_party_client(WEB_CLIENT_ID).unwrap();
        assert!(web.accepts_redirect("https://app.example.com/bff/callback"));
        for bad in [
            "https://app.example.com/bff/callback/",
            "https://app.example.com/bff/callback?x=1",
            "https://evil.example.com/bff/callback",
            "http://127.0.0.1:5000/callback",
        ] {
            assert!(!web.accepts_redirect(bad), "{bad}");
        }
    }

    #[test]
    fn native_accepts_any_loopback_port_and_its_scheme() {
        let clients = clients();
        let native = clients.first_party_client(NATIVE_CLIENT_ID).unwrap();
        for good in [
            "http://127.0.0.1:53124/callback",
            "http://127.0.0.1/callback",
            "http://[::1]:8000/callback",
            NATIVE_SCHEME_REDIRECT,
        ] {
            assert!(native.accepts_redirect(good), "{good}");
        }
        for bad in [
            "http://localhost:53124/callback",
            "https://127.0.0.1:53124/callback",
            "http://127.0.0.1:53124/other",
            "http://127.0.0.1:53124/callback?next=x",
            "http://user@127.0.0.1:1/callback",
            "http://10.0.0.1:1/callback",
            "http://127.0.0.1:1/callback#x",
        ] {
            assert!(!native.accepts_redirect(bad), "{bad}");
        }
        assert!(native.needs_consent() && !native.binds_workspace());
    }

    #[test]
    fn a_registered_loopback_redirect_matches_any_port_https_only_exactly() {
        let agent = public(&[
            "http://localhost:33418/callback",
            "https://agent.example.com/cb",
        ]);
        assert!(agent.accepts_redirect("http://localhost:5555/callback"));
        assert!(agent.accepts_redirect("https://agent.example.com/cb"));
        assert!(!agent.accepts_redirect("https://agent.example.com:8443/cb"));
        assert!(!agent.accepts_redirect("http://127.0.0.1:5555/callback"));
        assert!(!agent.accepts_redirect("http://localhost:5555/other"));
        assert!(is_https_redirect("https://agent.example.com/cb"));
        assert!(!is_https_redirect("http://localhost:5555/callback"));
        assert!(is_device_redirect("http://localhost:5555/callback"));
        assert!(is_device_redirect(NATIVE_SCHEME_REDIRECT));
        assert!(!is_device_redirect("https://agent.example.com/cb"));
    }

    #[test]
    fn identity_scopes_are_campbooks_own() {
        let catalogue = [
            ScopeInfo {
                name: "system:read",
                description: "",
            },
            ScopeInfo {
                name: "identity:read",
                description: "",
            },
        ];
        let agent = public(&[]);
        assert!(agent.may_ask_for("openid", &catalogue));
        assert!(agent.may_ask_for("system:read", &catalogue));
        assert!(
            !agent.may_ask_for("identity:read", &catalogue),
            "registered or not, never a third party's"
        );
        assert!(!agent.may_ask_for("mail:send", &catalogue), "unknown");
        let clients = clients();
        let native = clients.first_party_client(NATIVE_CLIENT_ID).unwrap();
        assert!(native.may_ask_for("identity:read", &catalogue));
        assert_eq!(third_party_scopes(&catalogue), ["system:read"]);
    }

    #[test]
    fn authentication_by_kind() {
        let clients = clients();
        let web = clients.first_party_client(WEB_CLIENT_ID).unwrap();
        assert!(web.authenticates(Some("s3cret")));
        assert!(!web.authenticates(Some("s3cre")));
        assert!(!web.authenticates(None));
        let native = clients.first_party_client(NATIVE_CLIENT_ID).unwrap();
        assert!(native.authenticates(None));
        assert!(!native.authenticates(Some("anything")));
        let mut workspace = public(&[]);
        workspace.authentication = Authentication::Hashed(hash("w-secret"));
        assert!(workspace.authenticates(Some("w-secret")));
        assert!(!workspace.authenticates(Some("w-secreT")));
        assert!(!workspace.authenticates(None));
        assert!(clients.first_party_client("other").is_none());
    }
}
