//! The OAuth client this instance is registered as at each provider: the
//! authorization request, the code exchange and ID-token validation, with
//! what differs between Google, Microsoft and Zoho kept as data.
//!
//! The ID token comes straight from the provider's token endpoint over TLS,
//! which OIDC allows to trust without a signature check; it is checked
//! anyway (signature against the provider's published keys, issuer,
//! audience, `azp`, expiry, nonce) through the kernel's verifier, so a
//! misdirected or forged response cannot sign anyone in.
//!
//! - **Google**: one issuer, one token endpoint, one key set.
//! - **Microsoft** (`common`, `organizations`, `consumers` or one tenant):
//!   one token endpoint and key set signing for every tenant's issuer,
//!   `https://login.microsoftonline.com/{tid}/v2.0`. The issuer a token must
//!   have is computed from its own `tid`, then checked with the signature.
//!   Its address counts as verified only with `xms_edov`.
//! - **Zoho**: the callback names the data centre's accounts server, which
//!   must be one of Zoho's (a fixed list) before anything is sent; the code
//!   is exchanged there, and its ID token must be issued by it.

use std::time::Duration;

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use campbooks_kernel::{auth::TokenVerifier, config::AuthConfig};
use secrecy::{ExposeSecret as _, SecretString};
use serde::Deserialize;
use url::Url;

use super::Provider;
use crate::{deps::ClientSecret, domain::accounts::ProviderIdentity};

/// Timeout of one request to a provider.
const HTTP_TIMEOUT: Duration = Duration::from_secs(10);
/// Largest response read from a provider.
const MAX_RESPONSE_BYTES: usize = 256 * 1024;

/// Microsoft's issuers: `{MICROSOFT_ISSUER_PREFIX}{tid}{MICROSOFT_ISSUER_SUFFIX}`.
pub const MICROSOFT_ISSUER_PREFIX: &str = "https://login.microsoftonline.com/";
/// See [`MICROSOFT_ISSUER_PREFIX`].
pub const MICROSOFT_ISSUER_SUFFIX: &str = "/v2.0";
/// The tenant of every personal Microsoft account.
pub const MICROSOFT_CONSUMERS_TENANT: &str = "9188040d-6c67-4c5b-b112-36a304b66dad";

/// The accounts server the authorization request goes to unless
/// `CAMPBOOKS_ZOHO__ACCOUNTS_SERVER` names another.
pub const ZOHO_DEFAULT_ACCOUNTS_SERVER: &str = "https://accounts.zoho.com";

/// Zoho's accounts servers, one per data centre. A callback naming any
/// other server is refused before a request is made.
pub const ZOHO_ACCOUNTS_SERVERS: [&str; 8] = [
    "https://accounts.zoho.com",
    "https://accounts.zoho.eu",
    "https://accounts.zoho.in",
    "https://accounts.zoho.com.au",
    "https://accounts.zoho.jp",
    "https://accounts.zoho.ca",
    "https://accounts.zoho.sa",
    "https://accounts.zoho.uk",
];

fn parse(url: &str) -> Url {
    Url::parse(url).unwrap_or_else(|e| unreachable!("a built-in URL does not parse: {e}"))
}

/// Where codes are redeemed and whose ID tokens are accepted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Server {
    /// The ID tokens' `iss` (for Microsoft, a placeholder: see
    /// [`IssuerRule::Tenant`]).
    pub issuer: String,
    /// The token endpoint.
    pub token: Url,
    /// The signing keys.
    pub jwks: Url,
}

/// Which issuer a provider's ID token must have.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IssuerRule {
    /// Exactly the server's.
    Fixed(Server),
    /// `{prefix}{tid}{suffix}` for the token's own `tid` (Microsoft), keys
    /// and token endpoint from `server`. With `only`, `tid` must be it;
    /// with `except`, it must not.
    Tenant {
        /// Token endpoint and keys.
        server: Server,
        /// Before the tenant id.
        prefix: String,
        /// After it.
        suffix: String,
        /// The only tenant accepted.
        only: Option<String>,
        /// A tenant refused.
        except: Option<String>,
    },
    /// The server the callback names, if it is one of these (Zoho).
    Servers(Vec<Server>),
}

/// A provider's endpoints.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoints {
    /// The authorization endpoint (the browser goes there).
    pub authorize: Url,
    /// Who issues the ID tokens, and where.
    pub issuers: IssuerRule,
}

impl Endpoints {
    /// Google's production endpoints.
    pub fn google() -> Self {
        Self {
            authorize: parse("https://accounts.google.com/o/oauth2/v2/auth"),
            issuers: IssuerRule::Fixed(Server {
                issuer: "https://accounts.google.com".to_owned(),
                token: parse("https://oauth2.googleapis.com/token"),
                jwks: parse("https://www.googleapis.com/oauth2/v3/certs"),
            }),
        }
    }

    /// Microsoft's endpoints for `tenant`: `common` (personal, work and
    /// school), `organizations`, `consumers`, or one tenant id.
    pub fn microsoft(tenant: &str) -> Result<Self, String> {
        Self::microsoft_under(
            "https://login.microsoftonline.com",
            MICROSOFT_ISSUER_PREFIX,
            tenant,
        )
    }

    /// Microsoft's endpoints laid out under `base` (a fake in development).
    fn microsoft_under(base: &str, prefix: &str, tenant: &str) -> Result<Self, String> {
        let (only, except) = match tenant {
            "common" => (None, None),
            "organizations" => (None, Some(MICROSOFT_CONSUMERS_TENANT.to_owned())),
            "consumers" => (Some(MICROSOFT_CONSUMERS_TENANT.to_owned()), None),
            id if is_tenant_id(id) => (Some(id.to_owned()), None),
            other => {
                return Err(format!(
                    "CAMPBOOKS_MICROSOFT__TENANT={other}: use common, organizations, consumers \
                     or a tenant id"
                ));
            }
        };
        let base = base.trim_end_matches('/');
        let url = |path: &str| {
            Url::parse(&format!("{base}/{tenant}/{path}")).map_err(|e| format!("{base}: {e}"))
        };
        Ok(Self {
            authorize: url("oauth2/v2.0/authorize")?,
            issuers: IssuerRule::Tenant {
                server: Server {
                    issuer: format!("{prefix}{tenant}{MICROSOFT_ISSUER_SUFFIX}"),
                    token: url("oauth2/v2.0/token")?,
                    jwks: url("discovery/v2.0/keys")?,
                },
                prefix: prefix.to_owned(),
                suffix: MICROSOFT_ISSUER_SUFFIX.to_owned(),
                only,
                except,
            },
        })
    }

    /// Zoho's endpoints: the authorization endpoint of `accounts_server`
    /// (one of [`ZOHO_ACCOUNTS_SERVERS`]; the default is
    /// [`ZOHO_DEFAULT_ACCOUNTS_SERVER`]) and every data centre's accounts
    /// server for the callback.
    pub fn zoho(accounts_server: &str) -> Result<Self, String> {
        if !ZOHO_ACCOUNTS_SERVERS.contains(&accounts_server) {
            return Err(format!(
                "{accounts_server} is not one of Zoho's accounts servers ({})",
                ZOHO_ACCOUNTS_SERVERS.join(", ")
            ));
        }
        Ok(Self {
            authorize: parse(&format!("{accounts_server}/oauth/v2/auth")),
            issuers: IssuerRule::Servers(
                ZOHO_ACCOUNTS_SERVERS
                    .iter()
                    .map(|server| zoho_server(server))
                    .collect(),
            ),
        })
    }

    /// `provider`'s endpoints under `base`, a fake laid out like the real
    /// one (development only): Google's issuer is `base`; Microsoft's
    /// issuers are `base/{tid}/v2.0`; Zoho has one accounts server, `base`.
    pub fn under(provider: Provider, base: &str, tenant: &str) -> Result<Self, String> {
        let issuer = base.trim_end_matches('/').to_owned();
        let join = |path: &str| {
            Url::parse(&format!("{issuer}/{path}")).map_err(|e| format!("{base}: {e}"))
        };
        Ok(match provider {
            Provider::Google => Self {
                authorize: join("o/oauth2/v2/auth")?,
                issuers: IssuerRule::Fixed(Server {
                    token: join("token")?,
                    jwks: join("oauth2/v3/certs")?,
                    issuer,
                }),
            },
            Provider::Microsoft => Self::microsoft_under(&issuer, &format!("{issuer}/"), tenant)?,
            Provider::Zoho => Self {
                authorize: join("oauth/v2/auth")?,
                issuers: IssuerRule::Servers(vec![Server {
                    token: join("oauth/v2/token")?,
                    jwks: join("oauth/v2/keys")?,
                    issuer,
                }]),
            },
        })
    }
}

fn zoho_server(origin: &str) -> Server {
    Server {
        issuer: origin.to_owned(),
        token: parse(&format!("{origin}/oauth/v2/token")),
        jwks: parse(&format!("{origin}/oauth/v2/keys")),
    }
}

/// A Microsoft tenant id: a GUID in lowercase hexadecimal.
pub fn is_tenant_id(value: &str) -> bool {
    value.len() == 36
        && value.char_indices().all(|(i, c)| match i {
            8 | 13 | 18 | 23 => c == '-',
            _ => c.is_ascii_digit() || ('a'..='f').contains(&c),
        })
}

/// What went wrong talking to a provider. Never carries a token, a code or
/// an address.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum UpstreamError {
    /// The provider refused the request (`invalid_grant`, `invalid_client`,
    /// …), or the callback named a server that is not the provider's.
    #[error("the provider refused the request: {0}")]
    Rejected(String),
    /// The provider could not be reached or answered with an error of its
    /// own.
    #[error("the provider is unavailable: {0}")]
    Unavailable(String),
    /// The ID token did not validate.
    #[error("the provider's ID token is invalid")]
    InvalidIdToken,
}

/// Tokens from a provider's token endpoint.
#[derive(Deserialize)]
pub struct Tokens {
    /// The access token.
    pub access_token: String,
    /// The refresh token: only with offline access (Google: only on a
    /// fresh consent).
    pub refresh_token: Option<String>,
    /// Seconds until the access token expires.
    pub expires_in: Option<u64>,
    /// The scopes granted, space-separated.
    pub scope: Option<String>,
    /// The ID token.
    pub id_token: Option<String>,
}

impl std::fmt::Debug for Tokens {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Tokens")
            .field("refresh_token", &self.refresh_token.is_some())
            .field("scope", &self.scope)
            .finish_non_exhaustive()
    }
}

#[derive(Deserialize)]
struct ErrorBody {
    error: Option<String>,
}

/// The provider-specific claims read from a validated ID token.
#[derive(Deserialize)]
struct ProviderClaims {
    /// Microsoft's tenant.
    tid: Option<String>,
    /// Microsoft's "email domain owner verified" optional claim.
    xms_edov: Option<serde_json::Value>,
}

/// An authorization request to a provider.
#[derive(Debug)]
pub struct AuthorizeRequest<'a> {
    /// Scopes (identity and mailbox), without the provider's own offline
    /// scope: see `offline`.
    pub scopes: &'a [String],
    /// `state`.
    pub state: &'a str,
    /// `nonce`.
    pub nonce: &'a str,
    /// The PKCE S256 challenge.
    pub code_challenge: &'a str,
    /// Ask for consent again (so a refresh token comes back), instead of
    /// only letting the person pick an account.
    pub consent: bool,
    /// Ask for a refresh token (only with mailbox scopes).
    pub offline: bool,
    /// The account to suggest.
    pub login_hint: Option<&'a str>,
}

struct Verified {
    server: Server,
    verifier: TokenVerifier,
}

enum Rule {
    Fixed(Verified),
    Tenant {
        verified: Verified,
        prefix: String,
        suffix: String,
        only: Option<String>,
        except: Option<String>,
    },
    Servers(Vec<Verified>),
}

/// The OAuth client this instance is registered as at one provider.
pub struct Upstream {
    provider: Provider,
    http: reqwest::Client,
    client_id: String,
    secret: ClientSecret,
    redirect_uri: String,
    authorize: Url,
    rule: Rule,
}

impl std::fmt::Debug for Upstream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Upstream")
            .field("provider", &self.provider)
            .field("client_id", &self.client_id)
            .field("redirect_uri", &self.redirect_uri)
            .finish_non_exhaustive()
    }
}

/// Who signed in, as the provider says, and what it granted.
#[derive(Debug)]
pub struct SignedIn {
    /// The provider identity.
    pub identity: ProviderIdentity,
    /// Its tokens.
    pub tokens: Tokens,
}

impl Upstream {
    /// `provider`'s client `client_id` with `secret`, sending `redirect_uri`
    /// (see [`super::redirect_uri`]) in both the authorization request and
    /// the code exchange, talking to `endpoints`.
    pub fn new(
        provider: Provider,
        client_id: String,
        secret: ClientSecret,
        redirect_uri: String,
        endpoints: Endpoints,
    ) -> Result<Self, String> {
        let http = reqwest::Client::builder()
            .timeout(HTTP_TIMEOUT)
            .redirect(reqwest::redirect::Policy::none())
            .user_agent(concat!("campbooks-server/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|e| format!("could not build the {} client: {e}", provider.as_str()))?;
        let verified = |server: Server| {
            let verifier = TokenVerifier::new(&AuthConfig {
                issuer: Some(server.issuer.clone()),
                audience: client_id.clone(),
                jwks_url: Some(server.jwks.to_string()),
                ..AuthConfig::default()
            });
            Verified { server, verifier }
        };
        let rule = match endpoints.issuers {
            IssuerRule::Fixed(server) => Rule::Fixed(verified(server)),
            IssuerRule::Tenant {
                server,
                prefix,
                suffix,
                only,
                except,
            } => Rule::Tenant {
                verified: verified(server),
                prefix,
                suffix,
                only,
                except,
            },
            IssuerRule::Servers(servers) => {
                Rule::Servers(servers.into_iter().map(verified).collect())
            }
        };
        Ok(Self {
            provider,
            http,
            redirect_uri,
            client_id,
            secret,
            authorize: endpoints.authorize,
            rule,
        })
    }

    /// The provider.
    pub fn provider(&self) -> Provider {
        self.provider
    }

    /// The `redirect_uri` sent to the provider.
    pub fn redirect_uri(&self) -> &str {
        &self.redirect_uri
    }

    /// The URL the browser is sent to.
    pub fn authorize_url(&self, request: &AuthorizeRequest<'_>) -> String {
        let mut scopes: Vec<&str> = request.scopes.iter().map(String::as_str).collect();
        if request.offline && self.provider == Provider::Microsoft {
            scopes.push("offline_access");
        }
        let mut url = self.authorize.clone();
        {
            let mut query = url.query_pairs_mut();
            query
                .append_pair("response_type", "code")
                .append_pair("client_id", &self.client_id)
                .append_pair("redirect_uri", &self.redirect_uri)
                .append_pair("scope", &scopes.join(" "))
                .append_pair("state", request.state)
                .append_pair("nonce", request.nonce)
                .append_pair("code_challenge", request.code_challenge)
                .append_pair("code_challenge_method", "S256");
            match self.provider {
                Provider::Google => {
                    if request.offline {
                        query.append_pair("access_type", "offline");
                    }
                    query.append_pair("include_granted_scopes", "true");
                    query.append_pair("prompt", prompt(request.consent));
                }
                Provider::Microsoft => {
                    query.append_pair("response_mode", "query");
                    query.append_pair("prompt", prompt(request.consent));
                }
                Provider::Zoho => {
                    if request.offline {
                        query.append_pair("access_type", "offline");
                    }
                    if request.consent {
                        query.append_pair("prompt", "consent");
                    }
                }
            }
            if let Some(hint) = request.login_hint {
                query.append_pair("login_hint", hint);
            }
        }
        url.into()
    }

    /// The server a callback's code is redeemed at: Zoho's must be named by
    /// the callback's `accounts-server` and be on the list; the others have
    /// one. `None` refuses the callback before any request.
    fn server(&self, data_centre: Option<&str>) -> Option<&Verified> {
        match &self.rule {
            Rule::Fixed(verified) | Rule::Tenant { verified, .. } => Some(verified),
            Rule::Servers(servers) => {
                // Only a position in the configured list comes out of the
                // callback's value: the server used is always one of ours.
                let named = data_centre?.trim_end_matches('/');
                let index = servers
                    .iter()
                    .position(|v| v.server.issuer.trim_end_matches('/') == named)?;
                servers.get(index)
            }
        }
    }

    /// Whether the callback's `iss` (RFC 9207), when sent, is the issuer of
    /// the server the code goes to.
    fn callback_issuer_matches(&self, server: &Verified, iss: Option<&str>) -> bool {
        let Some(iss) = iss else {
            return true;
        };
        match &self.rule {
            Rule::Tenant { prefix, suffix, .. } => iss
                .strip_prefix(prefix.as_str())
                .and_then(|rest| rest.strip_suffix(suffix.as_str()))
                .is_some_and(is_tenant_id),
            _ => iss == server.server.issuer,
        }
    }

    async fn secret(&self) -> Result<SecretString, UpstreamError> {
        match &self.secret {
            ClientSecret::Inline(secret) => Ok(secret.clone()),
            ClientSecret::File(path) => tokio::fs::read_to_string(path)
                .await
                .map(|s| SecretString::from(s.trim().to_owned()))
                .map_err(|e| {
                    UpstreamError::Unavailable(format!(
                        "reading the {} client secret file {}: {e}",
                        self.provider.as_str(),
                        path.display()
                    ))
                }),
        }
    }

    /// Redeem `code` (with the PKCE `verifier`) at the server the callback
    /// named (`data_centre`, Zoho's `accounts-server`) and validate the ID token for the
    /// round trip that sent `nonce`. `iss` is the callback's RFC 9207
    /// parameter, if any.
    pub async fn sign_in(
        &self,
        code: &str,
        verifier: &str,
        nonce: &str,
        data_centre: Option<&str>,
        iss: Option<&str>,
    ) -> Result<SignedIn, UpstreamError> {
        let server = self
            .server(data_centre)
            .ok_or_else(|| UpstreamError::Rejected("not one of the provider's servers".into()))?;
        if !self.callback_issuer_matches(server, iss) {
            return Err(UpstreamError::Rejected("another issuer".into()));
        }
        let tokens = self.exchange(&server.server.token, code, verifier).await?;
        let id_token = tokens
            .id_token
            .as_deref()
            .ok_or(UpstreamError::InvalidIdToken)?;
        let identity = self.identify(server, id_token, nonce).await?;
        Ok(SignedIn { identity, tokens })
    }

    async fn identify(
        &self,
        server: &Verified,
        token: &str,
        nonce: &str,
    ) -> Result<ProviderIdentity, UpstreamError> {
        let invalid = |_| UpstreamError::InvalidIdToken;
        let (id, claims, issuer) = if let Rule::Tenant {
            prefix,
            suffix,
            only,
            except,
            ..
        } = &self.rule
        {
            // Only to know which issuer to require: the token is then
            // validated in full against that issuer.
            let tid = unverified_tenant(token).ok_or(UpstreamError::InvalidIdToken)?;
            if only.as_ref().is_some_and(|only| *only != tid)
                || except.as_ref().is_some_and(|except| *except == tid)
            {
                return Err(UpstreamError::InvalidIdToken);
            }
            let issuer = format!("{prefix}{tid}{suffix}");
            let (id, claims) = server
                .verifier
                .verify_id_token_with::<ProviderClaims>(
                    token,
                    &self.client_id,
                    nonce,
                    Some(&issuer),
                )
                .await
                .map_err(invalid)?;
            if claims.tid.as_deref() != Some(tid.as_str()) {
                return Err(UpstreamError::InvalidIdToken);
            }
            (id, claims, issuer)
        } else {
            let (id, claims) = server
                .verifier
                .verify_id_token_with::<ProviderClaims>(token, &self.client_id, nonce, None)
                .await
                .map_err(invalid)?;
            (id, claims, server.server.issuer.clone())
        };
        let email_verified = match self.provider {
            // Microsoft does not verify `email`; only `xms_edov` says the
            // domain's owner did.
            Provider::Microsoft => {
                matches!(claims.xms_edov, Some(serde_json::Value::Bool(true)))
                    || matches!(&claims.xms_edov, Some(serde_json::Value::String(v)) if v == "true" || v == "1")
            }
            Provider::Google | Provider::Zoho => id.email_verified,
        };
        Ok(ProviderIdentity {
            provider: self.provider.as_str().to_owned(),
            issuer,
            subject: id.subject,
            email: id.email,
            email_verified,
            name: id.name,
        })
    }

    /// Revoke a refresh token Campbooks will not keep, at the server whose
    /// issuer is `issuer` (Zoho caps the refresh tokens a person holds per
    /// app, so one left behind could evict a mailbox connection's own).
    /// Only Zoho revokes one token alone; for the others this does nothing
    /// (revoking a Google token revokes the whole grant, a connected
    /// mailbox's too). In the request body first (RFC 7009), then as the
    /// query parameter Zoho documents.
    pub async fn revoke_unkept(
        &self,
        issuer: &str,
        refresh_token: &str,
    ) -> Result<(), UpstreamError> {
        if self.provider != Provider::Zoho {
            return Ok(());
        }
        let Rule::Servers(servers) = &self.rule else {
            return Ok(());
        };
        let issuer = issuer.trim_end_matches('/');
        let Some(server) = servers
            .iter()
            .find(|v| v.server.issuer.trim_end_matches('/') == issuer)
        else {
            return Err(UpstreamError::Rejected(
                "not one of the provider's servers".into(),
            ));
        };
        // Next to the token endpoint (Zoho: `/oauth/v2/token/revoke`).
        let url = Url::parse(&format!(
            "{}/revoke",
            server.server.token.as_str().trim_end_matches('/')
        ))
        .map_err(|e| UpstreamError::Unavailable(format!("revocation URL: {e}")))?;
        let secret = self.secret().await?;
        let body = self
            .http
            .post(url.clone())
            .header(reqwest::header::CONTENT_TYPE, FORM)
            .body(encode(&[
                ("token", refresh_token),
                ("client_id", self.client_id.as_str()),
                ("client_secret", secret.expose_secret()),
            ]))
            .send()
            .await
            .map_err(|e| UpstreamError::Unavailable(e.without_url().to_string()))?;
        if body.status().is_success() {
            return Ok(());
        }
        let mut query = url;
        query.query_pairs_mut().append_pair("token", refresh_token);
        let response = self
            .http
            .post(query)
            .header(reqwest::header::CONTENT_TYPE, FORM)
            .send()
            .await
            .map_err(|e| UpstreamError::Unavailable(e.without_url().to_string()))?;
        if response.status().is_success() {
            Ok(())
        } else {
            Err(UpstreamError::Rejected(format!(
                "revocation answered {}",
                response.status().as_u16()
            )))
        }
    }

    async fn exchange(
        &self,
        endpoint: &Url,
        code: &str,
        verifier: &str,
    ) -> Result<Tokens, UpstreamError> {
        let secret = self.secret().await?;
        let form = [
            ("grant_type", "authorization_code"),
            ("code", code),
            ("code_verifier", verifier),
            ("redirect_uri", self.redirect_uri.as_str()),
            ("client_id", self.client_id.as_str()),
            ("client_secret", secret.expose_secret()),
        ];
        let response = self
            .http
            .post(endpoint.clone())
            .header(reqwest::header::CONTENT_TYPE, FORM)
            .header(reqwest::header::ACCEPT, "application/json")
            .body(encode(&form))
            .send()
            .await
            .map_err(|e| UpstreamError::Unavailable(e.without_url().to_string()))?;
        let status = response.status();
        let body = read_limited(response).await?;
        let error = || {
            serde_json::from_slice::<ErrorBody>(&body)
                .ok()
                .and_then(|b| b.error)
        };
        if !status.is_success() {
            let error = error().unwrap_or_else(|| status.to_string());
            return Err(if status.is_client_error() {
                UpstreamError::Rejected(error)
            } else {
                UpstreamError::Unavailable(error)
            });
        }
        // Zoho answers some refusals with 200 and an `error`.
        if let Some(error) = error() {
            return Err(UpstreamError::Rejected(error));
        }
        serde_json::from_slice(&body)
            .map_err(|e| UpstreamError::Unavailable(format!("invalid token response: {e}")))
    }
}

const fn prompt(consent: bool) -> &'static str {
    if consent { "consent" } else { "select_account" }
}

/// The `tid` claim of a JWT, read without any check (only to choose the
/// issuer to check it against). `None` unless it is a tenant id.
fn unverified_tenant(token: &str) -> Option<String> {
    #[derive(Deserialize)]
    struct Tenant {
        tid: Option<String>,
    }
    let payload = token.split('.').nth(1)?;
    let bytes = URL_SAFE_NO_PAD.decode(payload).ok()?;
    let tenant: Tenant = serde_json::from_slice(&bytes).ok()?;
    tenant.tid.filter(|tid| is_tenant_id(tid))
}

const FORM: &str = "application/x-www-form-urlencoded";

fn encode(pairs: &[(&str, &str)]) -> String {
    url::form_urlencoded::Serializer::new(String::new())
        .extend_pairs(pairs.iter().copied())
        .finish()
}

async fn read_limited(response: reqwest::Response) -> Result<Vec<u8>, UpstreamError> {
    let body = response
        .bytes()
        .await
        .map_err(|e| UpstreamError::Unavailable(e.without_url().to_string()))?;
    if body.len() > MAX_RESPONSE_BYTES {
        return Err(UpstreamError::Unavailable("response too large".to_owned()));
    }
    Ok(body.to_vec())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    fn client(provider: Provider, endpoints: Endpoints) -> Upstream {
        Upstream::new(
            provider,
            "client".to_owned(),
            ClientSecret::Inline(SecretString::from("secret")),
            super::super::redirect_uri(provider, "https://app.example.com", None).unwrap(),
            endpoints,
        )
        .unwrap()
    }

    fn query(url: &str) -> (Url, HashMap<String, String>) {
        let url = Url::parse(url).unwrap();
        let query = url.query_pairs().into_owned().collect();
        (url, query)
    }

    fn request(offline: bool, consent: bool) -> AuthorizeRequest<'static> {
        static SCOPES: std::sync::LazyLock<Vec<String>> =
            std::sync::LazyLock::new(|| vec!["openid".to_owned(), "email".to_owned()]);
        AuthorizeRequest {
            scopes: &SCOPES,
            state: "st",
            nonce: "no",
            code_challenge: "ch",
            consent,
            offline,
            login_hint: None,
        }
    }

    #[test]
    fn google_asks_for_offline_access_only_with_a_mailbox() {
        let google = client(Provider::Google, Endpoints::google());
        let (url, q) = query(&google.authorize_url(&request(true, false)));
        assert_eq!(url.host_str(), Some("accounts.google.com"));
        assert_eq!(
            q["redirect_uri"],
            "https://app.example.com/auth/callback/google"
        );
        assert_eq!(q["code_challenge_method"], "S256");
        assert_eq!(q["access_type"], "offline");
        assert_eq!(q["include_granted_scopes"], "true");
        assert_eq!(q["prompt"], "select_account");
        assert!(!q.contains_key("client_secret"));

        let (_, q) = query(&google.authorize_url(&request(false, true)));
        assert!(!q.contains_key("access_type"));
        assert_eq!(q["prompt"], "consent");
    }

    #[test]
    fn microsoft_uses_its_tenant_and_offline_access_scope() {
        let microsoft = client(Provider::Microsoft, Endpoints::microsoft("common").unwrap());
        let (url, q) = query(&microsoft.authorize_url(&request(false, false)));
        assert_eq!(
            url.as_str().split('?').next(),
            Some("https://login.microsoftonline.com/common/oauth2/v2.0/authorize")
        );
        assert_eq!(q["scope"], "openid email");
        assert_eq!(q["response_mode"], "query");
        let (_, q) = query(&microsoft.authorize_url(&request(true, true)));
        assert_eq!(q["scope"], "openid email offline_access");
        assert_eq!(q["prompt"], "consent");

        let work = Endpoints::microsoft("organizations").unwrap();
        assert!(work.authorize.as_str().contains("/organizations/"));
        assert!(Endpoints::microsoft("not a tenant").is_err());
        assert!(Endpoints::microsoft("72f988bf-86f1-41af-91ab-2d7cd011db47").is_ok());
    }

    #[test]
    fn zoho_asks_at_its_global_endpoint_by_default() {
        let zoho = client(
            Provider::Zoho,
            Endpoints::zoho(ZOHO_DEFAULT_ACCOUNTS_SERVER).unwrap(),
        );
        let (url, q) = query(&zoho.authorize_url(&request(false, false)));
        assert_eq!(url.host_str(), Some("accounts.zoho.com"));
        assert!(!q.contains_key("access_type"));
        assert!(!q.contains_key("prompt"));
        let (_, q) = query(&zoho.authorize_url(&request(true, true)));
        assert_eq!(q["access_type"], "offline");
        assert_eq!(q["prompt"], "consent");
    }

    #[test]
    fn zoho_asks_at_the_configured_data_centre() {
        let zoho = client(
            Provider::Zoho,
            Endpoints::zoho("https://accounts.zoho.eu").unwrap(),
        );
        let redirect = zoho.authorize_url(&request(false, false));
        println!("sign-in sends the browser to: {redirect}");
        let (url, _) = query(&redirect);
        assert_eq!(url.host_str(), Some("accounts.zoho.eu"));
        assert_eq!(url.path(), "/oauth/v2/auth");
        // The callback may still name any data centre.
        assert!(zoho.server(Some("https://accounts.zoho.in")).is_some());
    }

    #[test]
    fn zoho_refuses_an_accounts_server_that_is_not_zohos() {
        for bad in [
            "https://accounts.example.com",
            "http://accounts.zoho.eu",
            "https://accounts.zoho.eu/oauth",
            "https://accounts.zoho.eu/",
            "https://accounts.zoho.eu.evil.example.com",
            "",
        ] {
            assert!(Endpoints::zoho(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn zoho_redeems_only_at_its_own_accounts_servers() {
        let zoho = client(
            Provider::Zoho,
            Endpoints::zoho(ZOHO_DEFAULT_ACCOUNTS_SERVER).unwrap(),
        );
        assert_eq!(
            zoho.server(Some("https://accounts.zoho.eu"))
                .map(|v| v.server.token.as_str()),
            Some("https://accounts.zoho.eu/oauth/v2/token")
        );
        for refused in [
            None,
            Some("https://evil.example.com"),
            Some("https://accounts.zoho.eu.evil.example.com"),
            Some("http://accounts.zoho.eu"),
            Some("https://accounts.zoho.com.cn"),
        ] {
            assert!(zoho.server(refused).is_none(), "{refused:?}");
        }
    }

    #[test]
    fn tenant_ids_are_lowercase_guids() {
        assert!(is_tenant_id(MICROSOFT_CONSUMERS_TENANT));
        assert!(!is_tenant_id("9188040D-6C67-4C5B-B112-36A304B66DAD"));
        assert!(!is_tenant_id("common"));
        assert!(!is_tenant_id("9188040d-6c67-4c5b-b112-36a304b66dad/../x"));
        let claims = URL_SAFE_NO_PAD.encode(br#"{"tid":"9188040d-6c67-4c5b-b112-36a304b66dad"}"#);
        assert_eq!(
            unverified_tenant(&format!("h.{claims}.s")).as_deref(),
            Some(MICROSOFT_CONSUMERS_TENANT)
        );
        let bad = URL_SAFE_NO_PAD.encode(br#"{"tid":"x/../../evil"}"#);
        assert_eq!(unverified_tenant(&format!("h.{bad}.s")), None);
    }

    #[test]
    fn fakes_are_laid_out_under_their_base() {
        let google =
            Endpoints::under(Provider::Google, "http://127.0.0.1:9000/", "common").unwrap();
        assert_eq!(
            google.authorize.as_str(),
            "http://127.0.0.1:9000/o/oauth2/v2/auth"
        );
        let IssuerRule::Fixed(server) = google.issuers else {
            panic!("google has one issuer");
        };
        assert_eq!(server.issuer, "http://127.0.0.1:9000");
        assert_eq!(server.token.as_str(), "http://127.0.0.1:9000/token");

        let microsoft =
            Endpoints::under(Provider::Microsoft, "http://127.0.0.1:9051", "common").unwrap();
        assert_eq!(
            microsoft.authorize.as_str(),
            "http://127.0.0.1:9051/common/oauth2/v2.0/authorize"
        );
        let IssuerRule::Tenant { prefix, .. } = microsoft.issuers else {
            panic!("microsoft is multi-tenant");
        };
        assert_eq!(prefix, "http://127.0.0.1:9051/");

        let zoho = Endpoints::under(Provider::Zoho, "http://127.0.0.1:9052", "common").unwrap();
        let IssuerRule::Servers(servers) = zoho.issuers else {
            panic!("zoho has servers");
        };
        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].issuer, "http://127.0.0.1:9052");
    }
}
