//! Instance apps (campbooks-cloud#369): confidential sign-in clients of the
//! whole instance, for the operator's own other apps (first-party single
//! sign-on, such as Applicamp).
//!
//! An instance administrator registers, lists, rotates and removes them
//! with `campbooks-server admin apps`, on the **migration login**, and
//! nothing else does: no HTTP endpoint calls these functions, and a trigger
//! refuses the runtime role any change to an instance app but recording
//! when it last got a token (migration `identity__instance_apps`).
//!
//! An instance app:
//!
//! - authenticates at the token endpoint with its secret
//!   (`client_secret_basic` or `client_secret_post`), a 256-bit random
//!   value stored as its SHA-256 only and shown once, when created or
//!   rotated;
//! - uses the authorization code with PKCE (S256), and refresh tokens when
//!   granted `offline_access`;
//! - matches redirect URIs exactly: `https` with a host, or loopback `http`
//!   on a development instance, written in their canonical form;
//! - may ask for at most its **ceiling**, scopes it was registered with
//!   from the OIDC scopes and the API catalogue a third party may hold (never
//!   `identity:*`, `workspaces:write`, `workspaces:admin` or `admin:access`);
//!   asking for more gets the intersection;
//! - is shown on the consent screen once per person, bound to the workspace
//!   they pick, like any app that is not Campbooks' own; revocable from
//!   Settings → Apps and integrations.
//!
//! Every change is recorded in `identity.audit_events` and tracked (ADR
//! 0020) as `system` with the command's name, in the transaction of the
//! change.

use campbooks_kernel::{
    Error, Result,
    auth::ScopeInfo,
    crypto::{hash, random_token},
    tracking::Tracker,
};
use chrono::{DateTime, Utc};
use sqlx::{Connection as _, PgConnection};

use crate::{
    domain::{
        accounts, client_names,
        clients::{self, third_party_may_hold},
        tracked,
    },
    metadata::MAX_REDIRECT_URIS,
    protocol::discovery::OIDC_SCOPES,
    tracking,
};

/// The prefix of every instance app's client id, followed by 16 random
/// bytes in unpadded base64url.
pub const CLIENT_ID_PREFIX: &str = "app_";

/// The longest redirect URI kept.
const MAX_URI: usize = 2048;

/// What an instance administrator registers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewInstanceApp {
    /// The name the consent screen shows.
    pub name: String,
    /// Where codes may go: exact strings.
    pub redirect_uris: Vec<String>,
    /// Where the browser may go after RP-initiated logout.
    pub post_logout_redirect_uris: Vec<String>,
    /// The ceiling: every scope it may ever be granted.
    pub scopes: Vec<String>,
}

/// Why an instance app was refused. Read by the operator on the terminal.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Refusal {
    /// Empty, or longer than [`client_names::MAX_NAME`] characters.
    #[error("the name must be 1 to {max} visible characters", max = client_names::MAX_NAME)]
    Name,
    /// The name reads as Campbooks' own.
    #[error("the name reads as Campbooks or Not A Camp, which only Campbooks' own apps may use")]
    NameReserved,
    /// One word of the name mixes alphabets.
    #[error("a word of the name mixes alphabets (a look-alike letter)")]
    NameMixesScripts,
    /// No redirect URI, or more than [`MAX_REDIRECT_URIS`].
    #[error("give 1 to {max} redirect URIs", max = MAX_REDIRECT_URIS)]
    RedirectCount,
    /// More than [`MAX_REDIRECT_URIS`] post-logout redirect URIs.
    #[error("give at most {max} post-logout redirect URIs", max = MAX_REDIRECT_URIS)]
    PostLogoutCount,
    /// A URI that is not `https` with a host (or loopback `http` on a
    /// development instance), has a fragment or userinfo, or is too long.
    #[error(
        "`{0}` is not allowed: use https with a host (loopback http only on a development \
         instance), no fragment, no user or password, at most 2048 characters"
    )]
    Uri(String),
    /// A URI not written as its canonical form, so an exact match would
    /// surprise.
    #[error("write `{given}` as `{canonical}`: redirect URIs match exactly")]
    NotCanonical {
        /// What the operator wrote.
        given: String,
        /// How it reads once parsed.
        canonical: String,
    },
    /// The same URI twice.
    #[error("`{0}` is given twice")]
    Duplicate(String),
    /// No scope.
    #[error("give at least one scope (openid, to sign people in)")]
    NoScope,
    /// A scope outside what an instance app may hold.
    #[error(
        "`{0}` may not be in an instance app's ceiling: only openid, profile, email, \
         offline_access and the API scopes a third party may hold"
    )]
    Scope(String),
}

/// An instance app as `admin apps list` shows it. Never its secret.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct InstanceApp {
    /// Its client id.
    pub client_id: String,
    /// Its name.
    pub name: String,
    /// Its redirect URIs.
    pub redirect_uris: Vec<String>,
    /// Where the browser may go after logout.
    pub post_logout_redirect_uris: Vec<String>,
    /// Its ceiling.
    pub allowed_scopes: Vec<String>,
    /// When it was registered.
    pub created_at: DateTime<Utc>,
    /// When its secret last changed.
    pub secret_rotated_at: Option<DateTime<Utc>>,
    /// When it last got a token.
    pub last_used_at: Option<DateTime<Utc>>,
}

const COLUMNS: &str = "client_id, name, redirect_uris, post_logout_redirect_uris, allowed_scopes, \
     created_at, secret_rotated_at, last_used_at";

/// `app`, checked and cleaned: its name cleaned of invisible characters, its
/// scopes without repeats. `catalogue` is the API's scope catalogue;
/// `development` lets loopback `http` redirects in.
pub fn validate(
    app: &NewInstanceApp,
    catalogue: &[ScopeInfo],
    development: bool,
) -> std::result::Result<NewInstanceApp, Refusal> {
    let name = client_names::clean(&app.name);
    if name.is_empty() || app.name.trim().chars().count() > client_names::MAX_NAME {
        return Err(Refusal::Name);
    }
    if client_names::impersonates_campbooks(&name) {
        return Err(Refusal::NameReserved);
    }
    if client_names::mixes_scripts(&name) {
        return Err(Refusal::NameMixesScripts);
    }
    if !(1..=MAX_REDIRECT_URIS).contains(&app.redirect_uris.len()) {
        return Err(Refusal::RedirectCount);
    }
    if app.post_logout_redirect_uris.len() > MAX_REDIRECT_URIS {
        return Err(Refusal::PostLogoutCount);
    }
    for list in [&app.redirect_uris, &app.post_logout_redirect_uris] {
        for (i, uri) in list.iter().enumerate() {
            check_uri(uri, development)?;
            if list[..i].contains(uri) {
                return Err(Refusal::Duplicate(uri.clone()));
            }
        }
    }
    let mut scopes: Vec<String> = Vec::new();
    for scope in &app.scopes {
        if !may_hold(scope, catalogue) {
            return Err(Refusal::Scope(scope.clone()));
        }
        if !scopes.contains(scope) {
            scopes.push(scope.clone());
        }
    }
    if scopes.is_empty() {
        return Err(Refusal::NoScope);
    }
    Ok(NewInstanceApp {
        name,
        redirect_uris: app.redirect_uris.clone(),
        post_logout_redirect_uris: app.post_logout_redirect_uris.clone(),
        scopes,
    })
}

/// Whether an instance app's ceiling may hold `scope`: an OIDC scope, or a
/// scope of `catalogue` a third party may hold.
pub fn may_hold(scope: &str, catalogue: &[ScopeInfo]) -> bool {
    OIDC_SCOPES.contains(&scope)
        || (catalogue.iter().any(|s| s.name == scope) && third_party_may_hold(scope))
}

/// Whether `uri` may be one of an instance app's redirect URIs.
fn check_uri(uri: &str, development: bool) -> std::result::Result<(), Refusal> {
    let refused = || Refusal::Uri(uri.to_owned());
    if uri.len() > MAX_URI {
        return Err(refused());
    }
    let url = url::Url::parse(uri).map_err(|_| refused())?;
    if url.fragment().is_some() || !url.username().is_empty() || url.password().is_some() {
        return Err(refused());
    }
    let allowed =
        clients::is_https_redirect(uri) || (development && clients::is_loopback_redirect(uri));
    if !allowed {
        return Err(refused());
    }
    if url.as_str() != uri {
        return Err(Refusal::NotCanonical {
            given: uri.to_owned(),
            canonical: url.into(),
        });
    }
    Ok(())
}

/// A fresh client id: [`CLIENT_ID_PREFIX`] and 16 random bytes.
fn new_client_id() -> Result<String> {
    let mut id = [0_u8; 16];
    aws_lc_rs::rand::fill(&mut id).map_err(|_| Error::internal("no entropy"))?;
    Ok(format!(
        "{CLIENT_ID_PREFIX}{}",
        base64::Engine::encode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, id)
    ))
}

/// Register `app` (already [`validate`]d) on `conn`, the migration login:
/// the app and its secret, shown this once. Tracks
/// `identity.instance_app_registered`.
pub async fn register(
    conn: &mut PgConnection,
    tracker: &Tracker,
    app: &NewInstanceApp,
    now: DateTime<Utc>,
) -> Result<(InstanceApp, String)> {
    let client_id = new_client_id()?;
    let secret = random_token();
    let mut tx = conn.begin().await?;
    clients::as_server(&mut tx).await?;
    let stored: InstanceApp = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "INSERT INTO identity.clients
             (client_id, kind, workspace_id, name, redirect_uris, post_logout_redirect_uris,
              grant_types, auth_method, secret_hash, allowed_scopes, created_by, created_at)
         VALUES ($1, 'instance', NULL, $2, $3, $4, '{{authorization_code,refresh_token}}',
                 'client_secret_basic', $5, $6, NULL, $7)
         RETURNING {COLUMNS}"
    )))
    .bind(&client_id)
    .bind(&app.name)
    .bind(&app.redirect_uris)
    .bind(&app.post_logout_redirect_uris)
    .bind(hash(&secret))
    .bind(&app.scopes)
    .bind(now)
    .fetch_one(&mut *tx)
    .await?;
    accounts::audit(
        &mut tx,
        None,
        "instance_app_registered",
        serde_json::json!({ "client": client_id, "kind": "instance", "via": "cli" }),
        now,
    )
    .await?;
    let client = tracked::client(&mut tx, &client_id).await?;
    tracker
        .track(&mut tx, &tracking::InstanceAppRegistered { client })
        .await?;
    tx.commit().await?;
    Ok((stored, secret))
}

/// Every instance app, oldest first.
pub async fn list(conn: &mut PgConnection) -> Result<Vec<InstanceApp>> {
    let mut tx = conn.begin().await?;
    clients::as_server(&mut tx).await?;
    let apps = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {COLUMNS} FROM identity.clients WHERE kind = 'instance'
          ORDER BY created_at, client_id"
    )))
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(apps)
}

/// A new secret for instance app `id`, shown this once; the old one stops
/// working at once. `None` when there is no such instance app. Tracks
/// `identity.instance_app_secret_rotated`.
pub async fn rotate_secret(
    conn: &mut PgConnection,
    tracker: &Tracker,
    id: &str,
    now: DateTime<Utc>,
) -> Result<Option<(InstanceApp, String)>> {
    let secret = random_token();
    let mut tx = conn.begin().await?;
    clients::as_server(&mut tx).await?;
    let rotated: Option<InstanceApp> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "UPDATE identity.clients SET secret_hash = $2, secret_rotated_at = $3
          WHERE client_id = $1 AND kind = 'instance'
          RETURNING {COLUMNS}"
    )))
    .bind(id)
    .bind(hash(&secret))
    .bind(now)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(rotated) = rotated else {
        return Ok(None);
    };
    accounts::audit(
        &mut tx,
        None,
        "instance_app_secret_rotated",
        serde_json::json!({ "client": id, "via": "cli" }),
        now,
    )
    .await?;
    let client = tracked::client(&mut tx, id).await?;
    tracker
        .track(&mut tx, &tracking::InstanceAppSecretRotated { client })
        .await?;
    tx.commit().await?;
    Ok(Some((rotated, secret)))
}

/// What [`remove`] ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Removed {
    /// Live consents people had given it.
    pub consents_revoked: i64,
    /// Live refresh-token families (sign-ins) it held.
    pub refresh_tokens_revoked: i64,
}

/// Remove instance app `id`, revoking every consent people gave it and
/// every refresh token it holds; its access tokens stop at once, as they
/// carry the consent they stand on, and its codes waiting to be redeemed
/// find no client. `None` when there is no such instance app. Tracks `identity.instance_app_removed`.
pub async fn remove(
    conn: &mut PgConnection,
    tracker: &Tracker,
    id: &str,
    now: DateTime<Utc>,
) -> Result<Option<Removed>> {
    let mut tx = conn.begin().await?;
    clients::as_server(&mut tx).await?;
    let gone =
        sqlx::query("DELETE FROM identity.clients WHERE client_id = $1 AND kind = 'instance'")
            .bind(id)
            .execute(&mut *tx)
            .await?
            .rows_affected();
    if gone != 1 {
        return Ok(None);
    }
    let consents: Vec<uuid::Uuid> = sqlx::query_scalar(
        "UPDATE identity.consents SET revoked_at = $2
          WHERE client_id = $1 AND revoked_at IS NULL
          RETURNING account_id",
    )
    .bind(id)
    .bind(now)
    .fetch_all(&mut *tx)
    .await?;
    let families = sqlx::query(
        "UPDATE identity.refresh_families SET revoked_at = $2, revoke_reason = 'client_removed'
          WHERE client_id = $1 AND revoked_at IS NULL",
    )
    .bind(id)
    .bind(now)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    let removed = Removed {
        consents_revoked: i64::try_from(consents.len()).unwrap_or(i64::MAX),
        refresh_tokens_revoked: i64::try_from(families).unwrap_or(i64::MAX),
    };
    accounts::audit(
        &mut tx,
        None,
        "instance_app_removed",
        serde_json::json!({
            "client": id,
            "via": "cli",
            "consents_revoked": removed.consents_revoked,
            "refresh_tokens_revoked": removed.refresh_tokens_revoked,
        }),
        now,
    )
    .await?;
    tracker
        .track(
            &mut tx,
            &tracking::InstanceAppRemoved {
                client_id: id.to_owned(),
                consents_revoked: removed.consents_revoked,
                refresh_tokens_revoked: removed.refresh_tokens_revoked,
            },
        )
        .await?;
    tx.commit().await?;
    Ok(Some(removed))
}

#[cfg(test)]
mod tests {
    use super::*;

    const CATALOGUE: [ScopeInfo; 4] = [
        ScopeInfo {
            name: "conversations:read",
            description: "",
        },
        ScopeInfo {
            name: "identity:read",
            description: "",
        },
        ScopeInfo {
            name: "workspaces:write",
            description: "",
        },
        ScopeInfo {
            name: "admin:access",
            description: "",
        },
    ];

    fn app() -> NewInstanceApp {
        NewInstanceApp {
            name: "Applicamp".to_owned(),
            redirect_uris: vec!["https://applicamp.example.com/auth/callback".to_owned()],
            post_logout_redirect_uris: vec!["https://applicamp.example.com/".to_owned()],
            scopes: vec![
                "openid".to_owned(),
                "profile".to_owned(),
                "email".to_owned(),
                "offline_access".to_owned(),
                "conversations:read".to_owned(),
                "openid".to_owned(),
            ],
        }
    }

    #[test]
    fn a_good_app_is_cleaned_and_kept() {
        let checked = validate(&app(), &CATALOGUE, false).unwrap();
        assert_eq!(
            checked.scopes,
            [
                "openid",
                "profile",
                "email",
                "offline_access",
                "conversations:read"
            ]
        );
        let mut spaced = app();
        spaced.name = "  Appli\u{200B}camp  ".to_owned();
        assert_eq!(
            validate(&spaced, &CATALOGUE, false).unwrap().name,
            "Applicamp"
        );
    }

    #[test]
    fn the_ceiling_holds_oidc_and_third_party_catalogue_scopes_only() {
        for scope in [
            "identity:read",
            "workspaces:write",
            "admin:access",
            "mail:send",
            "",
        ] {
            let mut asked = app();
            asked.scopes.push(scope.to_owned());
            assert_eq!(
                validate(&asked, &CATALOGUE, false),
                Err(Refusal::Scope(scope.to_owned())),
                "{scope}"
            );
        }
        let mut none = app();
        none.scopes.clear();
        assert_eq!(validate(&none, &CATALOGUE, false), Err(Refusal::NoScope));
    }

    #[test]
    fn redirects_are_exact_https_and_loopback_only_in_development() {
        let with = |uri: &str| {
            let mut asked = app();
            asked.redirect_uris = vec![uri.to_owned()];
            asked
        };
        for bad in [
            "http://applicamp.example.com/cb",
            "https://applicamp.example.com/cb#x",
            "https://user@applicamp.example.com/cb",
            "com.example.app:/cb",
            "https://",
            "not a url",
        ] {
            assert_eq!(
                validate(&with(bad), &CATALOGUE, true),
                Err(Refusal::Uri(bad.to_owned())),
                "{bad}"
            );
        }
        let loopback = "http://127.0.0.1:4000/auth/callback";
        assert_eq!(
            validate(&with(loopback), &CATALOGUE, false),
            Err(Refusal::Uri(loopback.to_owned()))
        );
        assert!(validate(&with(loopback), &CATALOGUE, true).is_ok());
        assert_eq!(
            validate(&with("https://Applicamp.example.com/cb"), &CATALOGUE, false),
            Err(Refusal::NotCanonical {
                given: "https://Applicamp.example.com/cb".to_owned(),
                canonical: "https://applicamp.example.com/cb".to_owned(),
            })
        );
        let mut twice = app();
        twice.redirect_uris.push(twice.redirect_uris[0].clone());
        assert!(matches!(
            validate(&twice, &CATALOGUE, false),
            Err(Refusal::Duplicate(_))
        ));
        let mut none = app();
        none.redirect_uris.clear();
        assert_eq!(
            validate(&none, &CATALOGUE, false),
            Err(Refusal::RedirectCount)
        );
        let mut logout = app();
        logout.post_logout_redirect_uris = vec!["http://applicamp.example.com/".to_owned()];
        assert!(matches!(
            validate(&logout, &CATALOGUE, false),
            Err(Refusal::Uri(_))
        ));
    }

    #[test]
    fn names_go_through_the_look_alike_checks() {
        for (name, refusal) in [
            ("", Refusal::Name),
            ("\u{200B}", Refusal::Name),
            ("Campbooks", Refusal::NameReserved),
            ("Not A Camp", Refusal::NameReserved),
        ] {
            let mut asked = app();
            asked.name = name.to_owned();
            assert_eq!(validate(&asked, &CATALOGUE, false), Err(refusal), "{name}");
        }
        let mut long = app();
        long.name = "a".repeat(101);
        assert_eq!(validate(&long, &CATALOGUE, false), Err(Refusal::Name));
    }

    #[test]
    fn client_ids_are_minted_like_the_others() {
        let id = new_client_id().unwrap();
        assert!(id.starts_with(CLIENT_ID_PREFIX));
        assert_eq!(
            campbooks_kernel::tracking::minted_client_id(&id),
            Some(id.as_str())
        );
        assert_ne!(id, new_client_id().unwrap());
    }
}
