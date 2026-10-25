//! Microsoft: the identity platform's token endpoint and Microsoft Graph,
//! as this module calls them. The only code here that speaks HTTP to
//! Microsoft (campbooks-cloud#315; reading mail is #316).
//!
//! The consent itself is the identity module's front door. This refreshes a
//! connection's access token at the mail tenant's token endpoint and reads
//! Graph `/me`, which names the mailbox a grant reaches. Hosts are
//! constants (`login.microsoftonline.com`, `graph.microsoft.com`), the
//! tenant a checked setting; nothing in a request or an answer builds a
//! URL. Calls follow no redirect, time out after [`CALL_TIMEOUT`] and read
//! at most [`MAX_ANSWER_BYTES`]. Errors never carry a token, a code or an
//! answer's body.

use std::time::Duration;

use secrecy::ExposeSecret as _;
use serde::Deserialize;
use url::Url;

use crate::provider::{
    ClientSecret, FORM, ProviderError, TokenGrant, form_body, http_client, retry_after, transport,
    unreadable,
};

/// The Graph scopes the `messages` capability needs: read and change mail,
/// send it, and read the account's profile (`/me`). Asked for with
/// `offline_access`, which the identity module adds to a mail consent.
pub const MAIL_SCOPES: [&str; 3] = ["Mail.ReadWrite", "Mail.Send", "User.Read"];

/// The scope that brings a refresh token.
pub const OFFLINE_SCOPE: &str = "offline_access";

/// How Microsoft may prefix a Graph scope in a token answer.
const GRAPH_RESOURCE: &str = "https://graph.microsoft.com/";

/// The identity platform's host: every tenant's token endpoint is under it.
pub const LOGIN_ORIGIN: &str = "https://login.microsoftonline.com";

/// Microsoft Graph, up to its version.
pub const GRAPH_BASE: &str = "https://graph.microsoft.com/v1.0/";

/// The longest one call to Microsoft may take.
pub const CALL_TIMEOUT: Duration = Duration::from_secs(15);

/// The largest answer read from Microsoft (a token answer or `/me` is a
/// few kilobytes).
pub const MAX_ANSWER_BYTES: usize = 64 * 1024;

/// The tenant of a mail connection when neither
/// `CAMPBOOKS_CONNECTORS__MICROSOFT__TENANT` nor sign-in's
/// `CAMPBOOKS_MICROSOFT__TENANT` names one (sign-in's own default): personal,
/// work and school accounts.
pub const DEFAULT_TENANT: &str = "common";

/// AADSTS errors that say the grant is gone, and only the person can bring
/// it back: the refresh token is invalid (70000), expired (70008), expired
/// for inactivity (700082) or by a password change (50173), the consent was
/// withdrawn (65001), multi-factor authentication is now required (50076,
/// 50079), the account is disabled (50057), or Conditional Access blocks it
/// (53003).
pub const GRANT_GONE_CODES: [u64; 9] = [
    70000, 70008, 700_082, 50173, 65001, 50076, 50079, 50057, 53003,
];

/// OAuth errors that say the same.
const GRANT_GONE_ERRORS: [&str; 4] = [
    "invalid_grant",
    "interaction_required",
    "consent_required",
    "login_required",
];

/// `scope` without Microsoft's Graph resource prefix.
fn scope_name(scope: &str) -> &str {
    match scope.get(..GRAPH_RESOURCE.len()) {
        Some(prefix) if prefix.eq_ignore_ascii_case(GRAPH_RESOURCE) => {
            scope.get(GRAPH_RESOURCE.len()..).unwrap_or_default()
        }
        _ => scope,
    }
}

/// Whether two Microsoft scopes are the same: Microsoft may answer with or
/// without the Graph prefix, and in any case.
pub fn same_scope(a: &str, b: &str) -> bool {
    scope_name(a).eq_ignore_ascii_case(scope_name(b))
}

/// Whether `granted` holds every one of [`MAIL_SCOPES`].
pub fn mail_granted(granted: &[String]) -> bool {
    MAIL_SCOPES
        .iter()
        .all(|wanted| granted.iter().any(|scope| same_scope(scope, wanted)))
}

/// Whether `tenant` is one Microsoft's endpoints take: `common`,
/// `organizations`, `consumers`, or a tenant id (a GUID in lowercase
/// hexadecimal).
pub fn is_tenant(tenant: &str) -> bool {
    matches!(tenant, "common" | "organizations" | "consumers") || is_tenant_id(tenant)
}

/// A Microsoft tenant id: a GUID in lowercase hexadecimal.
fn is_tenant_id(value: &str) -> bool {
    value.len() == 36
        && value.char_indices().all(|(i, c)| match i {
            8 | 13 | 18 | 23 => c == '-',
            _ => c.is_ascii_digit() || ('a'..='f').contains(&c),
        })
}

/// Where Microsoft's services live.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoints {
    /// The mail tenant's token endpoint.
    pub token: Url,
    /// Microsoft Graph, up to `/v1.0/`.
    pub graph: Url,
}

impl Endpoints {
    /// Microsoft's production endpoints for `tenant` (see [`is_tenant`]).
    pub fn microsoft(tenant: &str) -> Result<Self, String> {
        Self::laid_out(LOGIN_ORIGIN, GRAPH_BASE, tenant)
    }

    /// Every endpoint under `base`, laid out like Microsoft's (a fake in
    /// tests and development): `base/{tenant}/oauth2/v2.0/token` and
    /// `base/v1.0/`.
    pub fn under(base: &Url, tenant: &str) -> Result<Self, String> {
        let base = base.as_str().trim_end_matches('/');
        Self::laid_out(base, &format!("{base}/v1.0/"), tenant)
    }

    fn laid_out(login: &str, graph: &str, tenant: &str) -> Result<Self, String> {
        if !is_tenant(tenant) {
            return Err(format!(
                "CAMPBOOKS_CONNECTORS__MICROSOFT__TENANT={tenant}: use common, organizations, \
                 consumers or a tenant id"
            ));
        }
        let parse = |url: &str| Url::parse(url).map_err(|e| format!("{url}: {e}"));
        Ok(Self {
            token: parse(&format!("{login}/{tenant}/oauth2/v2.0/token"))?,
            graph: parse(graph)?,
        })
    }
}

/// The mailbox Graph `/me` names, validated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphUser {
    /// The Graph user id: 1 to 64 letters, digits and `-` (a GUID for a
    /// work or school account, 16 hexadecimal digits for a personal one).
    pub id: String,
    /// Its primary address (`mail`, else `userPrincipalName`), trimmed and
    /// lower-cased.
    pub address: String,
    /// Its display name, trimmed, without control characters, at most 256
    /// characters.
    pub display_name: Option<String>,
}

/// The Microsoft OAuth client this instance is registered as (the one
/// Microsoft sign-in uses).
#[derive(Debug, Clone)]
pub struct Microsoft {
    http: reqwest::Client,
    client_id: String,
    client_secret: ClientSecret,
    endpoints: Endpoints,
}

impl Microsoft {
    /// A client with id `client_id` and secret `client_secret`, talking to
    /// `endpoints`.
    pub fn new(
        client_id: String,
        client_secret: ClientSecret,
        endpoints: Endpoints,
    ) -> Result<Self, String> {
        Ok(Self {
            http: http_client("Microsoft")?,
            client_id,
            client_secret,
            endpoints,
        })
    }

    /// A fresh access token for `refresh_token`, at the mail tenant's token
    /// endpoint. Microsoft rotates refresh tokens: a new one comes back with
    /// most answers.
    pub async fn refresh(&self, refresh_token: &str) -> Result<TokenGrant, ProviderError> {
        let secret = self
            .client_secret
            .read("CAMPBOOKS_MICROSOFT__CLIENT_SECRET_FILE")?;
        // The scopes asked for again: never more than the consent granted.
        let scope = std::iter::once(OFFLINE_SCOPE)
            .chain(MAIL_SCOPES)
            .collect::<Vec<_>>()
            .join(" ");
        let response = self
            .http
            .post(self.endpoints.token.clone())
            .timeout(CALL_TIMEOUT)
            .header(reqwest::header::CONTENT_TYPE, FORM)
            .header(reqwest::header::ACCEPT, "application/json")
            .body(form_body(&[
                ("grant_type", "refresh_token"),
                ("refresh_token", refresh_token),
                ("client_id", &self.client_id),
                ("client_secret", secret.expose_secret()),
                ("scope", &scope),
            ]))
            .send()
            .await
            .map_err(transport)?;
        let status = response.status();
        let headers = response.headers().clone();
        let body = bounded(response).await?;
        if !status.is_success() {
            return Err(token_failure(status, &headers, &body));
        }
        let grant: TokenGrant = serde_json::from_slice(&body)
            .map_err(|e| ProviderError::Unexpected(unreadable("the token answer", &e)))?;
        if grant.access_token.is_empty() {
            return Err(ProviderError::Unexpected(
                "the token answer has no access token".to_owned(),
            ));
        }
        Ok(grant)
    }

    /// The token endpoint refreshes go to (the mail tenant's).
    #[cfg(test)]
    pub fn token_endpoint(&self) -> &Url {
        &self.endpoints.token
    }

    /// The mailbox `access_token` reaches: Graph `/me`, validated.
    pub async fn me(&self, access_token: &str) -> Result<GraphUser, ProviderError> {
        let mut url = self
            .endpoints
            .graph
            .join("me")
            .map_err(|e| ProviderError::Unexpected(format!("the Graph URL: {e}")))?;
        url.query_pairs_mut()
            .append_pair("$select", "id,mail,userPrincipalName,displayName");
        let response = self
            .http
            .get(url)
            .timeout(CALL_TIMEOUT)
            .bearer_auth(access_token)
            .header(reqwest::header::ACCEPT, "application/json")
            .send()
            .await
            .map_err(transport)?;
        let status = response.status();
        let headers = response.headers().clone();
        let body = bounded(response).await?;
        if !status.is_success() {
            return Err(graph_failure(status, &headers, &body));
        }
        let answer: MeAnswer = serde_json::from_slice(&body)
            .map_err(|e| ProviderError::Unexpected(unreadable("Graph /me", &e)))?;
        answer.validated()
    }
}

/// Graph `/me`, as it answers.
#[derive(Deserialize)]
struct MeAnswer {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    mail: Option<String>,
    #[serde(default, rename = "userPrincipalName")]
    user_principal_name: Option<String>,
    #[serde(default, rename = "displayName")]
    display_name: Option<String>,
}

impl MeAnswer {
    fn validated(self) -> Result<GraphUser, ProviderError> {
        let id = self
            .id
            .filter(|id| {
                !id.is_empty()
                    && id.len() <= 64
                    && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
            })
            .ok_or_else(|| ProviderError::Unexpected("Graph /me named no user id".to_owned()))?;
        let address = [self.mail, self.user_principal_name]
            .into_iter()
            .flatten()
            .find_map(|candidate| crate::imap::discover::Address::parse(&candidate))
            .map(|parsed| parsed.address)
            .ok_or_else(|| {
                ProviderError::Unexpected("Graph /me named no usable address".to_owned())
            })?;
        let display_name = self
            .display_name
            .map(|name| {
                name.chars()
                    .filter(|c| !c.is_control())
                    .take(256)
                    .collect::<String>()
                    .trim()
                    .to_owned()
            })
            .filter(|name| !name.is_empty());
        Ok(GraphUser {
            id,
            address,
            display_name,
        })
    }
}

/// An answer's body, refused above [`MAX_ANSWER_BYTES`].
async fn bounded(mut response: reqwest::Response) -> Result<Vec<u8>, ProviderError> {
    let too_large = || ProviderError::Unexpected("the answer is too large".to_owned());
    let limit = u64::try_from(MAX_ANSWER_BYTES).unwrap_or(u64::MAX);
    if response
        .content_length()
        .is_some_and(|length| length > limit)
    {
        return Err(too_large());
    }
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(transport)? {
        if body.len() + chunk.len() > MAX_ANSWER_BYTES {
            return Err(too_large());
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

/// A token endpoint's error answer (RFC 6749 §5.2, with Microsoft's
/// `error_codes`).
#[derive(Deserialize)]
struct TokenError {
    #[serde(default)]
    error: String,
    #[serde(default)]
    error_codes: Vec<u64>,
    #[serde(default)]
    error_description: Option<String>,
}

impl TokenError {
    /// Whether it says the grant is gone (see [`GRANT_GONE_CODES`]).
    fn grant_gone(&self) -> bool {
        if GRANT_GONE_ERRORS.contains(&self.error.as_str())
            || self
                .error_codes
                .iter()
                .any(|code| GRANT_GONE_CODES.contains(code))
        {
            return true;
        }
        // Without `error_codes`, the description starts `AADSTS<code>:`.
        self.error_description
            .as_deref()
            .and_then(aadsts_code)
            .is_some_and(|code| GRANT_GONE_CODES.contains(&code))
    }
}

/// The code of a description that starts `AADSTS<code>:`.
fn aadsts_code(description: &str) -> Option<u64> {
    let rest = description.trim_start().strip_prefix("AADSTS")?;
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    if digits.is_empty() || digits.len() > 9 {
        return None;
    }
    digits.parse().ok()
}

/// A short, safe code from an answer (letters, digits and `_` only, at most
/// 40): never more of the answer than that.
fn safe_code(code: &str) -> String {
    code.chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
        .take(40)
        .collect()
}

/// Whether `status` with `headers` asks to slow down: 429, or a 503 with
/// `Retry-After` (Graph throttling).
fn throttled(status: reqwest::StatusCode, headers: &reqwest::header::HeaderMap) -> bool {
    status == reqwest::StatusCode::TOO_MANY_REQUESTS
        || (status == reqwest::StatusCode::SERVICE_UNAVAILABLE && retry_after(headers).is_some())
}

/// What a token endpoint's non-success answer means.
fn token_failure(
    status: reqwest::StatusCode,
    headers: &reqwest::header::HeaderMap,
    body: &[u8],
) -> ProviderError {
    if throttled(status, headers) {
        return ProviderError::RateLimited {
            retry_after: retry_after(headers),
        };
    }
    let answer = serde_json::from_slice::<TokenError>(body).ok();
    if answer.as_ref().is_some_and(TokenError::grant_gone) {
        return ProviderError::InvalidGrant;
    }
    if status.is_server_error() {
        return ProviderError::Unavailable(format!("HTTP {}", status.as_u16()));
    }
    let code = answer.map_or_else(String::new, |a| safe_code(&a.error));
    ProviderError::Unexpected(if code.is_empty() {
        format!("HTTP {}", status.as_u16())
    } else {
        format!("HTTP {} ({code})", status.as_u16())
    })
}

/// What a Graph non-success answer means.
fn graph_failure(
    status: reqwest::StatusCode,
    headers: &reqwest::header::HeaderMap,
    body: &[u8],
) -> ProviderError {
    #[derive(Deserialize)]
    struct Answer {
        error: Detail,
    }
    #[derive(Deserialize)]
    struct Detail {
        #[serde(default)]
        code: String,
    }
    if throttled(status, headers) {
        return ProviderError::RateLimited {
            retry_after: retry_after(headers),
        };
    }
    match status {
        reqwest::StatusCode::UNAUTHORIZED => ProviderError::Unauthorized,
        reqwest::StatusCode::NOT_FOUND => ProviderError::NotFound,
        s if s.is_server_error() => ProviderError::Unavailable(format!("HTTP {}", s.as_u16())),
        s => {
            let code = serde_json::from_slice::<Answer>(body)
                .map_or_else(|_| String::new(), |a| safe_code(&a.error.code));
            ProviderError::Unexpected(if code.is_empty() {
                format!("HTTP {}", s.as_u16())
            } else {
                format!("HTTP {} ({code})", s.as_u16())
            })
        }
    }
}

#[cfg(test)]
mod tests;
