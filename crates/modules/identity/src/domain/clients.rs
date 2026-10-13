//! The clients that may ask the issuer for tokens.
//!
//! Slice S1a has the two first-party clients only. They are defined in code,
//! not stored, because their redirect URIs follow `CAMPBOOKS_PUBLIC_URL` and
//! the web client's secret is derived from the instance secret: a fresh
//! install needs no provisioning step. Clients a workspace registers, and
//! dynamically registered ones, arrive with slice S4.

use aws_lc_rs::constant_time;
use secrecy::{ExposeSecret as _, SecretString};

/// The web app's client: the server's own BFF, a confidential client.
pub const WEB_CLIENT_ID: &str = "campbooks-web";
/// The native apps' client (Tauri): a public client using PKCE.
pub const NATIVE_CLIENT_ID: &str = "campbooks-native";
/// The redirect URI path of the native client on loopback, any port
/// (RFC 8252 §7.3).
pub const NATIVE_LOOPBACK_PATH: &str = "/callback";
/// The native client's private-use URI scheme redirect (RFC 8252 §7.1).
pub const NATIVE_SCHEME_REDIRECT: &str = "com.notacamp.campbooks:/oauth/callback";

/// How a client authenticates at the token endpoint.
pub enum Authentication {
    /// `client_secret_basic` or `client_secret_post`.
    Secret(SecretString),
    /// A public client: PKCE only.
    None,
}

/// One client.
pub struct Client {
    /// `client_id`.
    pub id: &'static str,
    /// Whether it is one of Campbooks' own apps (the web BFF, the native
    /// app). Only first-party clients may be granted `admin:access`
    /// (ADR 0015); every client registered by a workspace or dynamically is
    /// not.
    pub first_party: bool,
    /// Whether it may get codes and tokens. The native client is defined but
    /// off until native sign-in ships with its own safeguards (slice S4): a
    /// signed-in browser would otherwise hand a code to any local process
    /// listening on a loopback port, with no one asked (RFC 8252 §8.6).
    pub active: bool,
    /// How it authenticates.
    pub authentication: Authentication,
    /// Exact redirect URIs it registered.
    pub redirect_uris: Vec<String>,
    /// Whether loopback redirects to [`NATIVE_LOOPBACK_PATH`] on any port
    /// are accepted.
    pub loopback_any_port: bool,
    /// Where it may send the browser after logout.
    pub post_logout_redirect_uris: Vec<String>,
    /// A refresh token unused this long stops working.
    pub refresh_idle: chrono::TimeDelta,
    /// Every refresh token of a sign-in stops working this long after it.
    pub refresh_absolute: chrono::TimeDelta,
}

impl Client {
    /// Whether `uri` is one of this client's redirect URIs: an exact match,
    /// or for a native client `http://127.0.0.1:<port>/callback` or
    /// `http://[::1]:<port>/callback` on any port.
    pub fn accepts_redirect(&self, uri: &str) -> bool {
        if self.redirect_uris.iter().any(|allowed| allowed == uri) {
            return true;
        }
        if !self.loopback_any_port {
            return false;
        }
        let Ok(url) = url::Url::parse(uri) else {
            return false;
        };
        let loopback = matches!(
            url.host(),
            Some(url::Host::Ipv4(ip)) if ip.is_loopback()
        ) || matches!(url.host(), Some(url::Host::Ipv6(ip)) if ip.is_loopback());
        url.scheme() == "http"
            && loopback
            && url.path() == NATIVE_LOOPBACK_PATH
            && url.query().is_none()
            && url.fragment().is_none()
            && url.username().is_empty()
            && url.password().is_none()
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
            _ => false,
        }
    }

    /// Whether the client may be granted `scope`: anything but
    /// `admin:access`, which only first-party clients get.
    pub fn may_request(&self, scope: &str) -> bool {
        self.first_party || scope != campbooks_kernel::admin::ACCESS_SCOPE
    }

    /// Whether it is a confidential client.
    pub fn is_confidential(&self) -> bool {
        matches!(self.authentication, Authentication::Secret(_))
    }
}

/// Every client the issuer knows.
pub struct Clients {
    clients: Vec<Client>,
}

impl Clients {
    /// The first-party clients of the instance at `origin`; the web client's
    /// secret is `web_secret`.
    pub fn first_party(origin: &str, web_secret: SecretString) -> Self {
        Self {
            clients: vec![
                Client {
                    id: WEB_CLIENT_ID,
                    first_party: true,
                    active: true,
                    authentication: Authentication::Secret(web_secret),
                    redirect_uris: vec![format!("{origin}/bff/callback")],
                    loopback_any_port: false,
                    post_logout_redirect_uris: vec![format!("{origin}/")],
                    refresh_idle: chrono::TimeDelta::days(14),
                    refresh_absolute: chrono::TimeDelta::days(30),
                },
                Client {
                    id: NATIVE_CLIENT_ID,
                    first_party: true,
                    active: false,
                    authentication: Authentication::None,
                    redirect_uris: vec![NATIVE_SCHEME_REDIRECT.to_owned()],
                    loopback_any_port: true,
                    post_logout_redirect_uris: Vec::new(),
                    refresh_idle: chrono::TimeDelta::days(30),
                    refresh_absolute: chrono::TimeDelta::days(90),
                },
            ],
        }
    }

    /// The active client `id`; an inactive one is as unknown.
    pub fn get(&self, id: &str) -> Option<&Client> {
        self.clients
            .iter()
            .find(|client| client.id == id && client.active)
    }

    /// The client `id`, active or not (for its redirect rules' tests).
    #[cfg(test)]
    fn any(&self, id: &str) -> Option<&Client> {
        self.clients.iter().find(|client| client.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clients() -> Clients {
        Clients::first_party("https://app.example.com", SecretString::from("s3cret"))
    }

    #[test]
    fn web_redirects_match_exactly() {
        let clients = clients();
        let web = clients.get(WEB_CLIENT_ID).unwrap();
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
        let native = clients.any(NATIVE_CLIENT_ID).unwrap();
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
        ] {
            assert!(!native.accepts_redirect(bad), "{bad}");
        }
    }

    #[test]
    fn only_first_party_clients_may_request_admin_access() {
        let clients = clients();
        let web = clients.get(WEB_CLIENT_ID).unwrap();
        assert!(web.first_party);
        assert!(web.may_request("admin:access"));
        let third_party = Client {
            id: "acme-app",
            first_party: false,
            active: true,
            authentication: Authentication::None,
            redirect_uris: vec!["https://acme.example.com/cb".to_owned()],
            loopback_any_port: false,
            post_logout_redirect_uris: Vec::new(),
            refresh_idle: chrono::TimeDelta::days(1),
            refresh_absolute: chrono::TimeDelta::days(1),
        };
        assert!(!third_party.may_request("admin:access"));
        assert!(third_party.may_request("conversations:read"));
        assert!(third_party.may_request("openid"));
    }

    #[test]
    fn authentication_by_kind() {
        let clients = clients();
        let web = clients.get(WEB_CLIENT_ID).unwrap();
        assert!(web.authenticates(Some("s3cret")));
        assert!(!web.authenticates(Some("s3cre")));
        assert!(!web.authenticates(None));
        let native = clients.any(NATIVE_CLIENT_ID).unwrap();
        assert!(native.authenticates(None));
        assert!(
            clients.get(NATIVE_CLIENT_ID).is_none(),
            "off until slice S4"
        );
        assert!(!native.authenticates(Some("anything")));
        assert!(clients.get("other").is_none());
    }
}
