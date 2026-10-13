//! Delivering the built web client next to the API (ADR 0006: one image
//! serves API and web).
//!
//! When `CAMPBOOKS_WEB_DIR` is set, requests no route answers fall through to
//! [`WebClient::respond`], which serves files from that directory with a
//! single-page-app fallback to `index.html`. The rules (see
//! `openspec/changes/serve-web-client/design.md`):
//!
//! - `/api`, `/api/*`, `/admin/v1`, `/admin/v1/*` (the admin API),
//!   `/bff`, `/bff/*` and `/openapi.json` are reserved for the API and never
//!   answered with web client files. `/admin` and its other pages are the
//!   web client's admin section.
//! - Only `GET`/`HEAD` are served; anything else is a 404.
//! - A missing file under `/assets/` is a 404 (hashed URLs must fail loudly).
//! - Any other missing file gets `index.html` only when the request accepts
//!   `text/html`, i.e. a browser navigation; everything else is a 404.
//!
//! Every 404 here is the usual `application/problem+json`.

use std::{
    convert::Infallible,
    path::{Path, PathBuf},
};

use axum::{
    body::Body,
    extract::Request,
    http::{HeaderMap, HeaderValue, Method, StatusCode, header},
    response::{IntoResponse, Response},
};
use campbooks_kernel::{Config, Error};
use tower::ServiceExt as _;
use tower_http::services::{ServeDir, ServeFile, fs::ServeFileSystemResponseBody};

/// URL prefix of the content-hashed files Vite emits.
pub const ASSETS_PREFIX: &str = "/assets/";

/// `Cache-Control` for content-hashed assets: their URL changes with their
/// content, so they never need revalidating.
pub const CACHE_IMMUTABLE: &str = "public, max-age=31536000, immutable";

/// `Cache-Control` for everything else (notably `index.html`): stored, but
/// revalidated on every load so a new release shows up at once.
pub const CACHE_REVALIDATE: &str = "no-cache";

/// Content-Security-Policy sent with every HTML document of the web client.
///
/// `style-src` has no `'unsafe-inline'`: the Vite build emits an external
/// stylesheet only, and React sets `style` props through the CSSOM, which CSP
/// does not govern.
pub const CONTENT_SECURITY_POLICY: &str = "default-src 'self'; script-src 'self'; \
     style-src 'self'; img-src 'self' data:; connect-src 'self'; object-src 'none'; \
     base-uri 'none'; frame-ancestors 'none'; form-action 'self'";

/// Name of the entry point every client-side route falls back to.
const INDEX_HTML: &str = "index.html";

/// The configured web client directory, if any. An empty value counts as
/// unset.
pub fn configured_dir(config: &Config) -> Option<&Path> {
    config
        .web_dir
        .as_deref()
        .filter(|dir| !dir.as_os_str().is_empty())
}

/// Startup check: the directory must hold the client's `index.html`, so a
/// mistyped path fails loudly instead of serving 404s.
pub fn check_dir(dir: &Path) -> Result<(), String> {
    if dir.join(INDEX_HTML).is_file() {
        Ok(())
    } else {
        Err(format!(
            "CAMPBOOKS_WEB_DIR={} does not contain {INDEX_HTML}; point it at the \
             built web client (clients/app/dist) or unset it to run API-only",
            dir.display()
        ))
    }
}

/// Whether `path` belongs to the API surface and must never be answered with
/// web client files.
pub fn is_reserved(path: &str) -> bool {
    path == "/openapi.json"
        || ["/api", "/admin/v1", "/bff"]
            .iter()
            .any(|prefix| path == *prefix || path.starts_with(&format!("{prefix}/")))
}

/// Serves one built web client directory.
#[derive(Debug, Clone)]
pub struct WebClient {
    files: ServeDir,
    index: PathBuf,
}

impl WebClient {
    /// A client served from `dir` (which should pass [`check_dir`]).
    pub fn new(dir: &Path) -> Self {
        Self {
            files: ServeDir::new(dir),
            index: dir.join(INDEX_HTML),
        }
    }

    /// Answer a request no API route matched.
    pub async fn respond(self, request: Request) -> Response {
        let path = request.uri().path().to_owned();
        if is_reserved(&path) || !matches!(*request.method(), Method::GET | Method::HEAD) {
            return Error::NotFound.into_response();
        }

        // Prepared up front because the original request is consumed below.
        let spa_fallback = (!path.starts_with(ASSETS_PREFIX) && accepts_html(request.headers()))
            .then(|| index_request(&request));

        let response = into_axum(self.files.oneshot(request).await);
        let response = if response.status() == StatusCode::NOT_FOUND {
            match spa_fallback {
                Some(index_request) => {
                    into_axum(ServeFile::new(&self.index).oneshot(index_request).await)
                }
                None => return Error::NotFound.into_response(),
            }
        } else {
            response
        };
        if response.status() == StatusCode::NOT_FOUND {
            // `index.html` vanished after startup; still answer like the API.
            return Error::NotFound.into_response();
        }
        with_web_headers(&path, response)
    }
}

/// Whether the client asked for HTML, i.e. this is a browser navigation.
fn accepts_html(headers: &HeaderMap) -> bool {
    headers
        .get_all(header::ACCEPT)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .any(|value| value.to_ascii_lowercase().contains("text/html"))
}

/// A body-less copy of `request` aimed at `index.html`, keeping the method and
/// the conditional headers so `HEAD` and `If-Modified-Since` still work.
fn index_request(request: &Request) -> Request {
    let mut index = Request::new(Body::empty());
    *index.method_mut() = request.method().clone();
    *index.version_mut() = request.version();
    index.headers_mut().clone_from(request.headers());
    index
}

fn into_axum(
    result: Result<axum::http::Response<ServeFileSystemResponseBody>, Infallible>,
) -> Response {
    match result {
        Ok(response) => response.map(Body::new),
        Err(never) => match never {},
    }
}

/// Cache policy by path, plus the CSP on HTML documents.
fn with_web_headers(path: &str, mut response: Response) -> Response {
    let status = response.status();
    if status.is_success() || status.is_redirection() {
        let cache = if path.starts_with(ASSETS_PREFIX) {
            CACHE_IMMUTABLE
        } else {
            CACHE_REVALIDATE
        };
        response
            .headers_mut()
            .insert(header::CACHE_CONTROL, HeaderValue::from_static(cache));
    }
    let is_html = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.starts_with("text/html"));
    if is_html {
        response.headers_mut().insert(
            header::CONTENT_SECURITY_POLICY,
            HeaderValue::from_static(CONTENT_SECURITY_POLICY),
        );
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reserved_paths() {
        for path in [
            "/api",
            "/api/",
            "/api/v1/nope",
            "/bff",
            "/bff/api/v1/x",
            "/admin/v1",
            "/admin/v1/system/overview",
            "/openapi.json",
        ] {
            assert!(is_reserved(path), "{path}");
        }
        for path in [
            "/",
            "/apis",
            "/bffs/x",
            "/some/route",
            "/assets/x.js",
            "/admin",
            "/admin/system",
            "/admin/v10",
        ] {
            assert!(!is_reserved(path), "{path}");
        }
    }

    #[test]
    fn html_navigations_are_detected() {
        let mut headers = HeaderMap::new();
        assert!(!accepts_html(&headers));
        headers.insert(header::ACCEPT, HeaderValue::from_static("*/*"));
        assert!(!accepts_html(&headers));
        headers.insert(
            header::ACCEPT,
            HeaderValue::from_static("text/html,application/xhtml+xml,*/*;q=0.8"),
        );
        assert!(accepts_html(&headers));
    }

    #[test]
    fn empty_web_dir_counts_as_unset() {
        let mut config = campbooks_kernel::testing::config("postgres://localhost/x");
        assert_eq!(configured_dir(&config), None);
        config.web_dir = Some(PathBuf::new());
        assert_eq!(configured_dir(&config), None);
        config.web_dir = Some(PathBuf::from("/app/web"));
        assert_eq!(configured_dir(&config), Some(Path::new("/app/web")));
    }

    #[test]
    fn check_dir_requires_index_html() {
        let missing = std::env::temp_dir().join("campbooks-web-check-does-not-exist");
        let err = check_dir(&missing).unwrap_err();
        assert!(err.contains("CAMPBOOKS_WEB_DIR"), "{err}");
    }
}
