//! The two cookies the BFF sets. Both are `HttpOnly` and hold only a random
//! token; the server keeps the rest.
//!
//! - **Session** (`__Host-campbooks_session`): `SameSite=Strict`, so no
//!   cross-site request ever carries it.
//! - **Login** (`__Host-campbooks_login`): ties a started sign-in to the
//!   browser that started it. It must come back on the IdP's redirect to
//!   `/bff/callback`, a cross-site navigation, so it is `SameSite=Lax`; it is
//!   single-use and short-lived.
//!
//! Over `https` both are `Secure` and `__Host-` prefixed (no `Domain`,
//! `Path=/`). When `CAMPBOOKS_PUBLIC_URL` is plain `http` (local
//! development), browsers refuse `Secure` and `__Host-` cookies there, so the
//! server drops both and says so once at startup.

use axum::http::{HeaderMap, HeaderValue, header};

/// Session cookie name over `https`.
pub const SESSION_COOKIE: &str = "__Host-campbooks_session";
/// Login cookie name over `https`.
pub const LOGIN_COOKIE: &str = "__Host-campbooks_login";

/// Cookie names and attributes for one public URL.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cookies {
    secure: bool,
}

/// `SameSite` attribute.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SameSite {
    Strict,
    Lax,
}

impl Cookies {
    /// `Secure` + `__Host-` cookies when `secure` (an `https` public URL).
    pub fn new(secure: bool) -> Self {
        Self { secure }
    }

    /// The session cookie's name.
    pub fn session_name(self) -> &'static str {
        name(SESSION_COOKIE, self.secure)
    }

    /// The login cookie's name.
    pub fn login_name(self) -> &'static str {
        name(LOGIN_COOKIE, self.secure)
    }

    /// The session token the request carries.
    pub fn session(self, headers: &HeaderMap) -> Option<String> {
        read(headers, self.session_name())
    }

    /// The login token the request carries.
    pub fn login(self, headers: &HeaderMap) -> Option<String> {
        read(headers, self.login_name())
    }

    /// `Set-Cookie` for a session token, kept for `max_age_seconds`.
    pub fn set_session(self, token: &str, max_age_seconds: i64) -> HeaderValue {
        self.build(
            self.session_name(),
            token,
            max_age_seconds,
            SameSite::Strict,
        )
    }

    /// `Set-Cookie` removing the session cookie.
    pub fn clear_session(self) -> HeaderValue {
        self.build(self.session_name(), "", 0, SameSite::Strict)
    }

    /// `Set-Cookie` for a login token, kept for `max_age_seconds`.
    pub fn set_login(self, token: &str, max_age_seconds: i64) -> HeaderValue {
        self.build(self.login_name(), token, max_age_seconds, SameSite::Lax)
    }

    /// `Set-Cookie` removing the login cookie.
    pub fn clear_login(self) -> HeaderValue {
        self.build(self.login_name(), "", 0, SameSite::Lax)
    }

    fn build(self, name: &str, value: &str, max_age: i64, same_site: SameSite) -> HeaderValue {
        let same_site = match same_site {
            SameSite::Strict => "Strict",
            SameSite::Lax => "Lax",
        };
        let secure = if self.secure { "; Secure" } else { "" };
        let cookie = format!(
            "{name}={value}; Path=/; Max-Age={}; HttpOnly; SameSite={same_site}{secure}",
            max_age.max(0)
        );
        // Names are constants and values are URL-safe base64.
        HeaderValue::from_str(&cookie)
            .unwrap_or_else(|_| unreachable!("cookie names and tokens are header-safe"))
    }
}

fn name(secure_name: &'static str, secure: bool) -> &'static str {
    if secure {
        secure_name
    } else {
        secure_name.trim_start_matches("__Host-")
    }
}

/// The value of cookie `name` in the request's `Cookie` headers. A name that
/// appears twice (a cookie planted for another path or a parent domain) is
/// treated as absent.
fn read(headers: &HeaderMap, name: &str) -> Option<String> {
    let mut values = headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .filter(|(key, _)| *key == name)
        .map(|(_, value)| value.trim());
    let value = values.next()?;
    if values.next().is_some() || value.is_empty() {
        return None;
    }
    Some(value.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(cookies: &[&'static str]) -> HeaderMap {
        let mut headers = HeaderMap::new();
        for cookie in cookies {
            headers.append(header::COOKIE, HeaderValue::from_static(cookie));
        }
        headers
    }

    #[test]
    fn secure_cookies_are_host_prefixed_and_strict() {
        let cookies = Cookies::new(true);
        assert_eq!(
            cookies.set_session("abc", 3600),
            "__Host-campbooks_session=abc; Path=/; Max-Age=3600; HttpOnly; SameSite=Strict; Secure"
        );
        assert_eq!(
            cookies.set_login("xyz", 600),
            "__Host-campbooks_login=xyz; Path=/; Max-Age=600; HttpOnly; SameSite=Lax; Secure"
        );
        assert_eq!(
            cookies.clear_session(),
            "__Host-campbooks_session=; Path=/; Max-Age=0; HttpOnly; SameSite=Strict; Secure"
        );
    }

    #[test]
    fn plain_http_drops_the_prefix_and_secure() {
        let cookies = Cookies::new(false);
        assert_eq!(cookies.session_name(), "campbooks_session");
        assert_eq!(
            cookies.set_session("abc", 60),
            "campbooks_session=abc; Path=/; Max-Age=60; HttpOnly; SameSite=Strict"
        );
        assert_eq!(
            cookies.clear_login(),
            "campbooks_login=; Path=/; Max-Age=0; HttpOnly; SameSite=Lax"
        );
    }

    #[test]
    fn reads_one_cookie_among_many() {
        let cookies = Cookies::new(true);
        let h = headers(&[
            "a=1; __Host-campbooks_session=tok",
            "__Host-campbooks_login=lg",
        ]);
        assert_eq!(cookies.session(&h).as_deref(), Some("tok"));
        assert_eq!(cookies.login(&h).as_deref(), Some("lg"));
        assert_eq!(cookies.session(&headers(&["campbooks_session=tok"])), None);
        // Duplicates and empty values count as absent.
        assert_eq!(
            cookies.session(&headers(&[
                "__Host-campbooks_session=a; __Host-campbooks_session=b"
            ])),
            None
        );
        assert_eq!(
            cookies.session(&headers(&["__Host-campbooks_session="])),
            None
        );
    }
}
