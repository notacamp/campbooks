//! HTTP pieces the identity endpoints share: the issuer's cookies, the CSRF
//! check of the sign-in screens' actions, `return_to` validation and OAuth
//! error responses.
//!
//! Cookies hold only random tokens (the server keeps their SHA-256). Over
//! `https` they are `Secure` and `__Host-` prefixed; over plain `http`
//! (local development) browsers refuse both, so they are dropped.
//!
//! - **Issuer session** (`__Host-campbooks_auth`): `SameSite=Lax`, because a
//!   client's authorization request arrives as a top-level navigation from
//!   another site and must find the person signed in.
//! - **Round trip** (`__Host-campbooks_upstream`): ties a provider round trip
//!   to the browser that started it; `Lax` so it comes back on the
//!   provider's redirect.
//! - **Sign-up** (`__Host-campbooks_signup`): ties a pending new account to
//!   that browser; `Strict`, it is only read by same-origin requests.
//! - **Browser** (`__Host-campbooks_browser`): ties authorization requests
//!   to the browser that made them; `Lax`, because they start as top-level
//!   navigations from the client's site.

use axum::{
    Json,
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};

/// The issuer session cookie over `https`.
pub const SESSION_COOKIE: &str = "__Host-campbooks_auth";
/// The provider round-trip cookie over `https`.
pub const UPSTREAM_COOKIE: &str = "__Host-campbooks_upstream";
/// The pending sign-up cookie over `https`.
pub const SIGN_UP_COOKIE: &str = "__Host-campbooks_signup";
/// The authorization-request binding cookie over `https`.
pub const BROWSER_COOKIE: &str = "__Host-campbooks_browser";

/// The header the sign-in screens send on every action (as the web BFF's).
pub const CSRF_HEADER: &str = "x-campbooks-csrf";

/// Longest accepted `return_to`.
const MAX_RETURN_TO: usize = 2048;

/// `SameSite` attribute.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SameSite {
    /// Same-site requests only.
    Strict,
    /// Also top-level cross-site navigations.
    Lax,
}

/// Cookie names and attributes for one public URL.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cookies {
    secure: bool,
}

impl Cookies {
    /// `Secure` + `__Host-` cookies when `secure` (an `https` public URL).
    pub const fn new(secure: bool) -> Self {
        Self { secure }
    }

    /// The name of cookie `secure_name` for this site.
    pub fn name(self, secure_name: &'static str) -> &'static str {
        if self.secure {
            secure_name
        } else {
            secure_name.trim_start_matches("__Host-")
        }
    }

    /// The value of cookie `secure_name` in the request. A name that appears
    /// twice (a cookie planted for another path or domain) counts as absent.
    pub fn read(self, headers: &HeaderMap, secure_name: &'static str) -> Option<String> {
        let name = self.name(secure_name);
        let mut values = headers
            .get_all(header::COOKIE)
            .iter()
            .filter_map(|value| value.to_str().ok())
            .flat_map(|value| value.split(';'))
            .filter_map(|pair| pair.trim().split_once('='))
            .filter(|(key, _)| *key == name)
            .map(|(_, value)| value.to_owned());
        match (values.next(), values.next()) {
            (Some(value), None) if !value.is_empty() => Some(value),
            _ => None,
        }
    }

    /// `Set-Cookie` for `value` under `secure_name`, kept for
    /// `max_age_seconds` (0 removes it).
    pub fn set(
        self,
        secure_name: &'static str,
        value: &str,
        max_age_seconds: i64,
        same_site: SameSite,
    ) -> HeaderValue {
        let same_site = match same_site {
            SameSite::Strict => "Strict",
            SameSite::Lax => "Lax",
        };
        let secure = if self.secure { "; Secure" } else { "" };
        let cookie = format!(
            "{}={value}; Path=/; Max-Age={}; HttpOnly; SameSite={same_site}{secure}",
            self.name(secure_name),
            max_age_seconds.max(0)
        );
        // Names are constants and values are URL-safe base64.
        HeaderValue::from_str(&cookie)
            .unwrap_or_else(|_| unreachable!("cookie names and tokens are header-safe"))
    }

    /// `Set-Cookie` removing cookie `secure_name`.
    pub fn clear(self, secure_name: &'static str, same_site: SameSite) -> HeaderValue {
        self.set(secure_name, "", 0, same_site)
    }
}

/// Whether an unsafe request passes the CSRF check: exactly one
/// `X-Campbooks-CSRF: 1`, and `Sec-Fetch-Site: same-origin` (or, from a
/// browser that sends no fetch metadata, `Origin` equal to `origin`).
pub fn csrf_passes(headers: &HeaderMap, origin: &str) -> bool {
    let single = |name: &str| {
        let mut values = headers.get_all(name).iter();
        match (values.next(), values.next()) {
            (Some(value), None) => value.to_str().ok(),
            _ => None,
        }
    };
    if single(CSRF_HEADER) != Some("1") {
        return false;
    }
    match single("sec-fetch-site") {
        Some(site) => site == "same-origin",
        None => single(header::ORIGIN.as_str()) == Some(origin),
    }
}

/// A same-origin path to land on: `/`-rooted, not protocol-relative, no
/// backslash, quotes, angle brackets, whitespace or control characters.
/// Missing means `/`; `None` when unacceptable.
pub fn return_to(value: Option<&str>) -> Option<String> {
    let Some(value) = value else {
        return Some("/".to_owned());
    };
    let acceptable = value.starts_with('/')
        && !value.starts_with("//")
        && value.len() <= MAX_RETURN_TO
        && value
            .bytes()
            .all(|b| b.is_ascii_graphic() && !b"\\\"'<>`".contains(&b));
    acceptable.then(|| value.to_owned())
}

/// `response` with `Cache-Control: no-store` (RFC 6749 §5.1) and
/// `Pragma: no-cache`.
pub fn no_store(mut response: Response) -> Response {
    let headers = response.headers_mut();
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert(header::PRAGMA, HeaderValue::from_static("no-cache"));
    response
}

/// A 303 to `location`.
pub fn see_other(location: &str) -> Response {
    let mut response = StatusCode::SEE_OTHER.into_response();
    if let Ok(value) = HeaderValue::from_str(location) {
        response.headers_mut().insert(header::LOCATION, value);
    }
    no_store(response)
}

/// `response` with `Set-Cookie: cookie` appended.
pub fn with_cookie(mut response: Response, cookie: HeaderValue) -> Response {
    response.headers_mut().append(header::SET_COOKIE, cookie);
    response
}

/// An OAuth error from the token or revocation endpoint (RFC 6749 §5.2).
pub fn oauth_error(status: StatusCode, error: &'static str) -> Response {
    let mut response = (status, Json(serde_json::json!({ "error": error }))).into_response();
    if status == StatusCode::UNAUTHORIZED {
        response.headers_mut().insert(
            header::WWW_AUTHENTICATE,
            HeaderValue::from_static(r#"Basic realm="campbooks""#),
        );
    }
    no_store(response)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ORIGIN: &str = "https://app.example.com";

    fn headers(pairs: &[(&'static str, &'static str)]) -> HeaderMap {
        let mut headers = HeaderMap::new();
        for (name, value) in pairs {
            headers.append(*name, HeaderValue::from_static(value));
        }
        headers
    }

    #[test]
    fn csrf_needs_the_header_and_the_same_origin() {
        assert!(csrf_passes(
            &headers(&[(CSRF_HEADER, "1"), ("sec-fetch-site", "same-origin")]),
            ORIGIN
        ));
        assert!(csrf_passes(
            &headers(&[(CSRF_HEADER, "1"), ("origin", ORIGIN)]),
            ORIGIN
        ));
        for case in [
            headers(&[("sec-fetch-site", "same-origin")]),
            headers(&[(CSRF_HEADER, "1"), ("sec-fetch-site", "cross-site")]),
            headers(&[(CSRF_HEADER, "1"), ("origin", "https://evil.example.com")]),
            headers(&[(CSRF_HEADER, "1"), (CSRF_HEADER, "1"), ("origin", ORIGIN)]),
        ] {
            assert!(!csrf_passes(&case, ORIGIN), "{case:?}");
        }
    }

    #[test]
    fn return_to_accepts_same_origin_paths_only() {
        assert_eq!(return_to(None).as_deref(), Some("/"));
        assert_eq!(return_to(Some("/w/1?x=2")).as_deref(), Some("/w/1?x=2"));
        for bad in [
            "//evil.example.com",
            "https://evil.example.com",
            "/\\evil",
            "/a b",
            "/<script>",
            "",
        ] {
            assert_eq!(return_to(Some(bad)), None, "{bad}");
        }
    }

    #[test]
    fn cookies_are_host_prefixed_over_https_only() {
        let secure = Cookies::new(true);
        let cookie = secure.set(SESSION_COOKIE, "t", 60, SameSite::Lax);
        assert_eq!(
            cookie,
            "__Host-campbooks_auth=t; Path=/; Max-Age=60; HttpOnly; SameSite=Lax; Secure"
        );
        let plain = Cookies::new(false);
        assert_eq!(
            plain.clear(SIGN_UP_COOKIE, SameSite::Strict),
            "campbooks_signup=; Path=/; Max-Age=0; HttpOnly; SameSite=Strict"
        );

        let mut request = HeaderMap::new();
        request.insert(
            header::COOKIE,
            HeaderValue::from_static("a=1; campbooks_auth=tok"),
        );
        assert_eq!(plain.read(&request, SESSION_COOKIE).as_deref(), Some("tok"));
        request.append(
            header::COOKIE,
            HeaderValue::from_static("campbooks_auth=other"),
        );
        assert_eq!(plain.read(&request, SESSION_COOKIE), None, "planted twice");
    }
}
