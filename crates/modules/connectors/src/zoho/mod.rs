//! Zoho: the OAuth client of the account's data centre and the Zoho Mail
//! REST API, as this module calls them, and the Zoho Mail adapter (read,
//! write back). The only code here that speaks HTTP to Zoho.
//!
//! A Zoho account lives in one data centre. Its grant names the accounts
//! server that vouched for it (the ID token's issuer, validated by the
//! identity module against Zoho's list); tokens are refreshed and revoked
//! there, and mail is served by the same centre's mail host
//! (`accounts.zoho.eu` → `mail.zoho.eu`). Hosts never come from a request
//! or a provider's answer: only an issuer on [`ACCOUNTS_SERVERS`] (or the
//! configured development fake) maps to one.

mod api;
#[cfg(test)]
mod flows;
pub mod html;
pub mod read;
pub mod state;
pub mod write;

use secrecy::ExposeSecret as _;
use url::Url;

pub use api::{Listed, OriginalFetch, ZohoMail};

use crate::provider::{
    ClientSecret, FORM, OAuthError, ProviderError, TokenGrant, form_body, http_client, transport,
};

/// The scopes the `messages` capability needs: list the login's mail
/// accounts, read folders, read and change messages, send. The Zoho OAuth
/// app must allow these besides `openid email profile`.
pub const MAIL_SCOPES: [&str; 3] = [
    "ZohoMail.accounts.READ",
    "ZohoMail.folders.ALL",
    "ZohoMail.messages.ALL",
];

/// Zoho's accounts servers, one per data centre (the identity module
/// accepts the same list).
pub const ACCOUNTS_SERVERS: [&str; 8] = [
    "https://accounts.zoho.com",
    "https://accounts.zoho.eu",
    "https://accounts.zoho.in",
    "https://accounts.zoho.com.au",
    "https://accounts.zoho.jp",
    "https://accounts.zoho.ca",
    "https://accounts.zoho.sa",
    "https://accounts.zoho.uk",
];

/// Where one data centre's services live.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataCentre {
    /// The accounts server (the issuer), without a trailing slash.
    pub accounts: String,
    /// Its token endpoint.
    pub token: Url,
    /// Its revocation endpoint.
    pub revoke: Url,
    /// The Mail API base, up to `/api/`.
    pub mail: Url,
}

/// Which data centres a grant may name.
#[derive(Debug, Clone)]
pub enum Centres {
    /// Zoho's own ([`ACCOUNTS_SERVERS`]).
    Zoho,
    /// One fake, laid out like a data centre under one base (development
    /// only): accounts server = the base, Mail API under `base/api/`.
    Fake(Url),
}

impl Centres {
    /// The data centre of `issuer`, when it is one of these.
    pub fn of(&self, issuer: &str) -> Option<DataCentre> {
        let issuer = issuer.trim().trim_end_matches('/');
        let (accounts, mail) = match self {
            Self::Zoho => {
                let accounts = ACCOUNTS_SERVERS.iter().find(|s| **s == issuer)?;
                let host = accounts.strip_prefix("https://accounts.")?;
                ((*accounts).to_owned(), format!("https://mail.{host}/api/"))
            }
            Self::Fake(base) => {
                let base = base.as_str().trim_end_matches('/');
                if base != issuer {
                    return None;
                }
                (base.to_owned(), format!("{base}/api/"))
            }
        };
        let parse = |url: &str| Url::parse(url).ok();
        Some(DataCentre {
            token: parse(&format!("{accounts}/oauth/v2/token"))?,
            revoke: parse(&format!("{accounts}/oauth/v2/token/revoke"))?,
            mail: parse(&mail)?,
            accounts,
        })
    }
}

/// The Zoho OAuth client this instance is registered as.
#[derive(Debug, Clone)]
pub struct Zoho {
    http: reqwest::Client,
    client_id: String,
    client_secret: ClientSecret,
    centres: Centres,
}

impl Zoho {
    /// A client with id `client_id` and secret `client_secret`, for the
    /// data centres `centres`.
    pub fn new(
        client_id: String,
        client_secret: ClientSecret,
        centres: Centres,
    ) -> Result<Self, String> {
        Ok(Self {
            http: http_client("Zoho")?,
            client_id,
            client_secret,
            centres,
        })
    }

    /// The data centre of a grant issued by `issuer`, when it is Zoho's.
    pub fn centre(&self, issuer: &str) -> Option<DataCentre> {
        self.centres.of(issuer)
    }

    /// The Mail API of `centre` with `access_token`, for mail account
    /// `account` (empty for calls about the login, like listing accounts).
    pub fn mail<'a>(
        &'a self,
        centre: &'a DataCentre,
        access_token: &'a str,
        account: &'a str,
    ) -> ZohoMail<'a> {
        ZohoMail::new(&self.http, &centre.mail, access_token, account)
    }

    /// A fresh access token for `refresh_token` at `centre`. Zoho answers
    /// a refused refresh with `invalid_code` (sometimes with a 200).
    pub async fn refresh(
        &self,
        centre: &DataCentre,
        refresh_token: &str,
    ) -> Result<TokenGrant, ProviderError> {
        let secret = self
            .client_secret
            .read("CAMPBOOKS_ZOHO__CLIENT_SECRET_FILE")?;
        let response = self
            .http
            .post(centre.token.clone())
            .header(reqwest::header::CONTENT_TYPE, FORM)
            .body(form_body(&[
                ("grant_type", "refresh_token"),
                ("refresh_token", refresh_token),
                ("client_id", &self.client_id),
                ("client_secret", secret.expose_secret()),
            ]))
            .send()
            .await
            .map_err(transport)?;
        let status = response.status();
        let body = response.bytes().await.map_err(transport)?;
        let error = serde_json::from_slice::<OAuthError>(&body)
            .ok()
            .map(|e| e.error);
        match error.as_deref() {
            Some("invalid_code" | "invalid_grant" | "invalid_token") => {
                return Err(ProviderError::InvalidGrant);
            }
            Some(other) if status.is_success() => {
                return Err(ProviderError::Unexpected(format!("token answer: {other}")));
            }
            _ => {}
        }
        if status.is_success() {
            return serde_json::from_slice(&body)
                .map_err(|e| ProviderError::Unexpected(format!("token answer: {e}")));
        }
        Err(api::classify(status, None, None))
    }

    /// Revoke `refresh_token` at `centre` (the whole grant). A token Zoho no
    /// longer knows counts as revoked.
    pub async fn revoke(
        &self,
        centre: &DataCentre,
        refresh_token: &str,
    ) -> Result<(), ProviderError> {
        let mut url = centre.revoke.clone();
        url.query_pairs_mut().append_pair("token", refresh_token);
        let response = self
            .http
            .post(url)
            .header(reqwest::header::CONTENT_TYPE, FORM)
            .send()
            .await
            .map_err(transport)?;
        let status = response.status();
        if status.is_success() || status == reqwest::StatusCode::BAD_REQUEST {
            return Ok(());
        }
        Err(api::classify(status, None, None))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_zoho_issuer_maps_to_its_data_centre() {
        let eu = Centres::Zoho.of("https://accounts.zoho.eu/").unwrap();
        assert_eq!(eu.accounts, "https://accounts.zoho.eu");
        assert_eq!(eu.mail.as_str(), "https://mail.zoho.eu/api/");
        assert_eq!(eu.token.as_str(), "https://accounts.zoho.eu/oauth/v2/token");
        assert_eq!(
            eu.revoke.as_str(),
            "https://accounts.zoho.eu/oauth/v2/token/revoke"
        );
        let au = Centres::Zoho.of("https://accounts.zoho.com.au").unwrap();
        assert_eq!(au.mail.as_str(), "https://mail.zoho.com.au/api/");
        for issuer in ACCOUNTS_SERVERS {
            assert!(Centres::Zoho.of(issuer).is_some(), "{issuer}");
        }
    }

    #[test]
    fn anything_else_has_no_data_centre() {
        for issuer in [
            "https://accounts.evil.example",
            "http://accounts.zoho.eu",
            "https://accounts.zoho.eu.evil.example",
            "https://accounts.google.com",
            "",
        ] {
            assert!(Centres::Zoho.of(issuer).is_none(), "{issuer}");
        }
        let fake = Centres::Fake(Url::parse("http://127.0.0.1:9052").unwrap());
        assert!(fake.of("https://accounts.zoho.eu").is_none());
        let at = fake.of("http://127.0.0.1:9052").unwrap();
        assert_eq!(at.mail.as_str(), "http://127.0.0.1:9052/api/");
    }

    /// Against a real Zoho account. Run with `CAMPBOOKS_ZOHO__CLIENT_ID`,
    /// `CAMPBOOKS_ZOHO__CLIENT_SECRET`, `CAMPBOOKS_TEST_ZOHO_ACCOUNTS_SERVER`
    /// and `CAMPBOOKS_TEST_ZOHO_REFRESH_TOKEN` (a test login's token granted
    /// the mail scopes) set: reads only, never writes or sends.
    /// `cargo test -p campbooks-connectors real_zoho -- --ignored`.
    #[tokio::test]
    #[ignore = "needs a real Zoho OAuth client and test account"]
    async fn real_zoho_refreshes_and_lists_accounts_and_folders() {
        let var = |name: &str| std::env::var(name).unwrap_or_else(|_| panic!("set {name}"));
        let zoho = Zoho::new(
            var("CAMPBOOKS_ZOHO__CLIENT_ID"),
            ClientSecret::Inline(secrecy::SecretString::from(var(
                "CAMPBOOKS_ZOHO__CLIENT_SECRET",
            ))),
            Centres::Zoho,
        )
        .unwrap();
        let centre = zoho
            .centre(&var("CAMPBOOKS_TEST_ZOHO_ACCOUNTS_SERVER"))
            .unwrap();
        let grant = zoho
            .refresh(&centre, &var("CAMPBOOKS_TEST_ZOHO_REFRESH_TOKEN"))
            .await
            .unwrap();
        let accounts = zoho
            .mail(&centre, &grant.access_token, "")
            .accounts()
            .await
            .unwrap();
        let first = accounts.first().expect("a mail account");
        let folders = zoho
            .mail(&centre, &grant.access_token, &first.account_id)
            .folders()
            .await
            .unwrap();
        assert!(folders.iter().any(|f| f.kind == "Inbox"));
    }
}
