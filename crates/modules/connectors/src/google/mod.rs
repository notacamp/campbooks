//! Google's OAuth 2.0 endpoints and the Gmail API, as this module calls
//! them. The only code here that speaks HTTP to Google.
//!
//! Endpoint URLs are constructor parameters ([`Endpoints`]): the server
//! passes Google's, tests a wiremock server's. Errors never carry a token,
//! a code or a response body (which may echo either).

mod calendar;
mod gmail;

use secrecy::ExposeSecret as _;

use crate::{
    domain::model::Capability,
    provider::{
        ClientSecret, FORM, OAuthError, ProviderError, TokenGrant, form_body, http_client,
        retry_after, transport,
    },
};
use serde::Deserialize;
use url::Url;

pub use calendar::{
    Calendar, CalendarEntry, CalendarError, EventTime, EventsQuery, GoogleEvent, Person,
};
pub use gmail::{Gmail, LabelInfo, MessageMeta, MessageRaw, RawFetch};

/// The scope the `messages` capability needs: read mail and change labels,
/// no permanent deletion.
pub const GMAIL_SCOPE: &str = "https://www.googleapis.com/auth/gmail.modify";

/// The scope of the `calendar` capability that reads and writes events.
pub const CALENDAR_EVENTS_SCOPE: &str = "https://www.googleapis.com/auth/calendar.events";

/// The scope of the `calendar` capability that lists the account's
/// calendars.
pub const CALENDAR_LIST_SCOPE: &str =
    "https://www.googleapis.com/auth/calendar.calendarlist.readonly";

/// The Google scope the mail capability needs (`None` for the others).
pub const fn capability_scope(capability: Capability) -> Option<&'static str> {
    match capability {
        Capability::Messages => Some(GMAIL_SCOPE),
        Capability::Calendar | Capability::Files | Capability::Bank => None,
    }
}

/// The Google scopes a capability needs, when the capability is offered:
/// `messages` and `calendar` today. Files come with their adapter.
pub const fn capability_scopes(capability: Capability) -> &'static [&'static str] {
    match capability {
        Capability::Messages => &[GMAIL_SCOPE],
        Capability::Calendar => &[CALENDAR_EVENTS_SCOPE, CALENDAR_LIST_SCOPE],
        Capability::Files | Capability::Bank => &[],
    }
}

/// The offered capabilities `scopes` grant: every scope of a capability
/// must be granted (Google lets a person leave a scope out).
pub fn granted_capabilities(scopes: &[String]) -> Vec<Capability> {
    [
        Capability::Messages,
        Capability::Calendar,
        Capability::Files,
        Capability::Bank,
    ]
    .into_iter()
    .filter(|&capability| {
        let needed = capability_scopes(capability);
        !needed.is_empty() && needed.iter().all(|n| scopes.iter().any(|s| s == n))
    })
    .collect()
}

/// Where Google's services live.
#[derive(Debug, Clone)]
pub struct Endpoints {
    /// The token endpoint.
    pub token: Url,
    /// The revocation endpoint.
    pub revoke: Url,
    /// The Gmail API base, up to `/gmail/v1/`.
    pub gmail: Url,
    /// The Gmail upload base, up to `/upload/gmail/v1/` (sending).
    pub gmail_upload: Url,
    /// The Calendar API base, up to `/calendar/v3/`.
    pub calendar: Url,
}

impl Endpoints {
    /// Google's production endpoints.
    pub fn google() -> Self {
        let parse = |url: &str| Url::parse(url).unwrap_or_else(|e| unreachable!("{url}: {e}"));
        Self {
            token: parse("https://oauth2.googleapis.com/token"),
            revoke: parse("https://oauth2.googleapis.com/revoke"),
            gmail: parse("https://gmail.googleapis.com/gmail/v1/"),
            gmail_upload: parse("https://gmail.googleapis.com/upload/gmail/v1/"),
            calendar: parse("https://www.googleapis.com/calendar/v3/"),
        }
    }

    /// Every endpoint under `base` (a fake Google in tests).
    pub fn under(base: &Url) -> Self {
        let join = |path: &str| {
            base.join(path)
                .unwrap_or_else(|e| unreachable!("{base} + {path}: {e}"))
        };
        Self {
            token: join("token"),
            revoke: join("revoke"),
            gmail: join("gmail/v1/"),
            gmail_upload: join("upload/gmail/v1/"),
            calendar: join("calendar/v3/"),
        }
    }
}

/// The Google OAuth client this instance is registered as.
#[derive(Debug, Clone)]
pub struct Google {
    http: reqwest::Client,
    client_id: String,
    client_secret: ClientSecret,
    endpoints: Endpoints,
}

impl Google {
    /// A client with id `client_id` and secret `client_secret`, talking to
    /// `endpoints`.
    pub fn new(
        client_id: String,
        client_secret: ClientSecret,
        endpoints: Endpoints,
    ) -> Result<Self, String> {
        let http = http_client("Google")?;
        Ok(Self {
            http,
            client_id,
            client_secret,
            endpoints,
        })
    }

    /// The Gmail API with `access_token`.
    pub fn gmail<'a>(&'a self, access_token: &'a str) -> Gmail<'a> {
        Gmail::new(
            &self.http,
            &self.endpoints.gmail,
            &self.endpoints.gmail_upload,
            access_token,
        )
    }

    /// The Calendar API with `access_token`.
    pub fn calendar<'a>(&'a self, access_token: &'a str) -> Calendar<'a> {
        Calendar::new(&self.http, &self.endpoints.calendar, access_token)
    }

    /// A fresh access token for `refresh_token`.
    pub async fn refresh(&self, refresh_token: &str) -> Result<TokenGrant, ProviderError> {
        let secret = self.secret()?;
        self.token_request(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
            ("client_id", &self.client_id),
            ("client_secret", secret.expose_secret()),
        ])
        .await
    }

    /// Revoke `token` (a refresh token revokes the whole grant). A token
    /// Google no longer knows counts as revoked.
    pub async fn revoke(&self, token: &str) -> Result<(), ProviderError> {
        let response = self
            .http
            .post(self.endpoints.revoke.clone())
            .header(reqwest::header::CONTENT_TYPE, FORM)
            .body(form_body(&[("token", token)]))
            .send()
            .await
            .map_err(transport)?;
        let status = response.status();
        if status.is_success() || status == reqwest::StatusCode::BAD_REQUEST {
            // 400 `invalid_token`: already revoked or expired.
            return Ok(());
        }
        Err(classify(status, response.headers(), None))
    }

    async fn token_request(&self, form: &[(&str, &str)]) -> Result<TokenGrant, ProviderError> {
        let response = self
            .http
            .post(self.endpoints.token.clone())
            .header(reqwest::header::CONTENT_TYPE, FORM)
            .body(form_body(form))
            .send()
            .await
            .map_err(transport)?;
        let status = response.status();
        if status.is_success() {
            return response
                .json()
                .await
                .map_err(|e| ProviderError::Unexpected(format!("token response: {e}")));
        }
        let headers = response.headers().clone();
        let error = response.json::<OAuthError>().await.ok().map(|e| e.error);
        if error.as_deref() == Some("invalid_grant") {
            return Err(ProviderError::InvalidGrant);
        }
        Err(classify(status, &headers, error.as_deref()))
    }

    fn secret(&self) -> Result<secrecy::SecretString, ProviderError> {
        self.client_secret
            .read("CAMPBOOKS_GOOGLE__CLIENT_SECRET_FILE")
    }
}

/// Read a JSON answer, or classify the failure.
async fn json<T: serde::de::DeserializeOwned>(
    response: reqwest::Response,
) -> Result<T, ProviderError> {
    let status = response.status();
    if status.is_success() {
        return response
            .json()
            .await
            .map_err(|e| ProviderError::Unexpected(format!("response: {}", e.without_url())));
    }
    Err(failure(response).await)
}

/// What a non-success answer means.
async fn failure(response: reqwest::Response) -> ProviderError {
    let status = response.status();
    let headers = response.headers().clone();
    let reason = error_reason(response).await;
    classify(status, &headers, reason.as_deref())
}

/// The `reason` of a Google API error body (`rateLimitExceeded`, …).
async fn error_reason(response: reqwest::Response) -> Option<String> {
    #[derive(Deserialize)]
    struct Body {
        error: ErrorBody,
    }
    #[derive(Deserialize)]
    struct ErrorBody {
        #[serde(default)]
        errors: Vec<Detail>,
        #[serde(default)]
        status: Option<String>,
    }
    #[derive(Deserialize)]
    struct Detail {
        reason: String,
    }
    let body: Body = response.json().await.ok()?;
    body.error
        .errors
        .into_iter()
        .next()
        .map(|d| d.reason)
        .or(body.error.status)
}

/// What a non-success status means.
fn classify(
    status: reqwest::StatusCode,
    headers: &reqwest::header::HeaderMap,
    reason: Option<&str>,
) -> ProviderError {
    let rate_limited_reason = matches!(
        reason,
        Some("rateLimitExceeded" | "userRateLimitExceeded" | "RESOURCE_EXHAUSTED")
    );
    if status == reqwest::StatusCode::TOO_MANY_REQUESTS
        || (status == reqwest::StatusCode::FORBIDDEN && rate_limited_reason)
    {
        return ProviderError::RateLimited {
            retry_after: retry_after(headers),
        };
    }
    match status {
        reqwest::StatusCode::UNAUTHORIZED => ProviderError::Unauthorized,
        reqwest::StatusCode::NOT_FOUND => ProviderError::NotFound,
        s if s.is_server_error() => ProviderError::Unavailable(format!("HTTP {}", s.as_u16())),
        s => ProviderError::Unexpected(format!(
            "HTTP {}{}",
            s.as_u16(),
            reason.map(|r| format!(" ({r})")).unwrap_or_default()
        )),
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use secrecy::SecretString;

    use super::*;

    /// Against a real Google account (staging, #53). Run with
    /// `CAMPBOOKS_GOOGLE__CLIENT_ID`, `CAMPBOOKS_GOOGLE__CLIENT_SECRET` and
    /// `CAMPBOOKS_TEST_GOOGLE_REFRESH_TOKEN` (a token of a test user granted
    /// `gmail.modify`) set:
    /// `cargo test -p campbooks-connectors real_google -- --ignored`.
    #[tokio::test]
    #[ignore = "needs a real Google OAuth client and test account"]
    async fn real_google_refreshes_and_reads_the_profile() {
        let var = |name: &str| std::env::var(name).unwrap_or_else(|_| panic!("set {name}"));
        let google = Google::new(
            var("CAMPBOOKS_GOOGLE__CLIENT_ID"),
            ClientSecret::Inline(SecretString::from(var("CAMPBOOKS_GOOGLE__CLIENT_SECRET"))),
            Endpoints::google(),
        )
        .unwrap();
        let grant = google
            .refresh(&var("CAMPBOOKS_TEST_GOOGLE_REFRESH_TOKEN"))
            .await
            .unwrap();
        let profile = google.gmail(&grant.access_token).profile().await.unwrap();
        assert!(!profile.history_id.is_empty());
        let labels = google.gmail(&grant.access_token).labels().await.unwrap();
        assert!(labels.iter().any(|label| label.id == "INBOX"));
    }

    #[test]
    fn statuses_are_classified() {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(reqwest::header::RETRY_AFTER, "120".parse().unwrap());
        assert_eq!(
            classify(reqwest::StatusCode::TOO_MANY_REQUESTS, &headers, None),
            ProviderError::RateLimited {
                retry_after: Some(Duration::from_mins(2))
            }
        );
        let none = reqwest::header::HeaderMap::new();
        assert_eq!(
            classify(
                reqwest::StatusCode::FORBIDDEN,
                &none,
                Some("userRateLimitExceeded")
            ),
            ProviderError::RateLimited { retry_after: None }
        );
        assert_eq!(
            classify(reqwest::StatusCode::UNAUTHORIZED, &none, None),
            ProviderError::Unauthorized
        );
        assert_eq!(
            classify(reqwest::StatusCode::NOT_FOUND, &none, None),
            ProviderError::NotFound
        );
        assert!(matches!(
            classify(reqwest::StatusCode::BAD_GATEWAY, &none, None),
            ProviderError::Unavailable(_)
        ));
        assert!(matches!(
            classify(
                reqwest::StatusCode::FORBIDDEN,
                &none,
                Some("insufficientPermissions")
            ),
            ProviderError::Unexpected(_)
        ));
    }
}
