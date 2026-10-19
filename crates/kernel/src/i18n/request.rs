//! The locale of a request: the [`LocalePreference`] port, the [`Locale`]
//! extractor and the [`localize`] middleware that renders problem+json in
//! it.

use std::{
    convert::Infallible,
    future::Future,
    pin::Pin,
    sync::{Arc, OnceLock},
};

use axum::{
    extract::{FromRequestParts, Request, State},
    http::{Extensions, HeaderMap, header, request::Parts},
    middleware::Next,
    response::Response,
};
use sqlx::PgPool;

use super::Locale;
use crate::{
    AppState, Error,
    auth::{AuthenticatedPrincipal, Principal, PrincipalKind, bearer_token},
    error::LocalizableProblem,
};

/// A future returned by a [`LocalePreference`] port.
pub type LocaleFuture<'a> =
    Pin<Box<dyn Future<Output = Result<Option<Locale>, Error>> + Send + 'a>>;

/// The port that answers "which language did this user choose?".
/// Implemented by the `workspaces` module (which owns users); the server
/// registers it with [`AppState::with_locales`].
pub trait LocalePreference: std::fmt::Debug + Send + Sync + 'static {
    /// The locale the user behind `principal` saved, or `None` when they
    /// have not chosen one (or are unknown). Only called for users.
    fn preferred<'a>(&'a self, db: &'a PgPool, principal: &'a Principal) -> LocaleFuture<'a>;
}

/// The port before any module provides one: nobody chose a locale.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoLocalePreference;

impl LocalePreference for NoLocalePreference {
    fn preferred<'a>(&'a self, _db: &'a PgPool, _principal: &'a Principal) -> LocaleFuture<'a> {
        Box::pin(async { Ok(None) })
    }
}

/// Shared handle to the registered [`LocalePreference`] port.
pub type LocalePreferencePort = Arc<dyn LocalePreference>;

/// The request's locale once resolved, shared by the extractor and every
/// [`localize`] layer of one request (a request extension).
#[derive(Debug, Clone, Default)]
struct Slot(Arc<OnceLock<Locale>>);

/// What resolution reads from a request, captured before the request is
/// handed on.
struct Inputs {
    principal: Option<Principal>,
    token: Option<String>,
    accept_language: Option<String>,
}

/// How long the localize layer waits for the caller's saved locale.
const LOOKUP_LIMIT: std::time::Duration = std::time::Duration::from_secs(2);

impl Inputs {
    fn of(headers: &HeaderMap, extensions: &Extensions) -> Self {
        Self {
            principal: extensions
                .get::<AuthenticatedPrincipal>()
                .map(|AuthenticatedPrincipal(principal)| principal.clone()),
            token: bearer_token(headers).ok().map(ToOwned::to_owned),
            accept_language: headers
                .get(header::ACCEPT_LANGUAGE)
                .and_then(|value| value.to_str().ok())
                .map(ToOwned::to_owned),
        }
    }

    /// [`resolve`](Self::resolve), within [`LOOKUP_LIMIT`]: past it, the
    /// `Accept-Language` locale (campbooks-cloud#265). The localize layer
    /// sits outside the request time limit, so a slow token check or saved
    /// locale lookup must not stretch a timed-out request past it.
    async fn resolve_within_limit(self, state: &AppState) -> Locale {
        let fallback = self
            .accept_language
            .as_deref()
            .and_then(Locale::from_accept_language)
            .unwrap_or_default();
        tokio::time::timeout(LOOKUP_LIMIT, self.resolve(state))
            .await
            .unwrap_or_else(|_| {
                tracing::warn!("the caller's saved locale took too long; using Accept-Language");
                fallback
            })
    }

    /// The user's saved locale, then `Accept-Language`, then `en`.
    async fn resolve(self, state: &AppState) -> Locale {
        let principal = match (self.principal, self.token) {
            (Some(principal), _) => Some(principal),
            (None, Some(token)) if state.auth.is_configured() => {
                state.auth.verify(&token).await.ok()
            }
            _ => None,
        };
        if let Some(principal) = principal.filter(|p| p.kind == PrincipalKind::User) {
            match state.locales.preferred(&state.db, &principal).await {
                Ok(Some(locale)) => return locale,
                Ok(None) => {}
                Err(error) => tracing::warn!(%error, "cannot read the user's locale"),
            }
        }
        self.accept_language
            .as_deref()
            .and_then(Locale::from_accept_language)
            .unwrap_or_default()
    }
}

/// The caller's locale: their saved choice, else the first language of
/// `Accept-Language` we ship (any `pt-*` is `pt-PT`, `es-*` is `es-ES`),
/// else `en`. Never rejects. Resolved on first use in a request (verifying
/// the token and reading the user's choice at most once).
impl FromRequestParts<AppState> for Locale {
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Infallible> {
        let slot = parts.extensions.get::<Slot>().cloned();
        if let Some(locale) = slot.as_ref().and_then(|slot| slot.0.get()) {
            return Ok(*locale);
        }
        let locale = Inputs::of(&parts.headers, &parts.extensions)
            .resolve(state)
            .await;
        if let Some(slot) = slot {
            // A concurrent resolution of the same request agrees.
            let _ = slot.0.set(locale);
        }
        Ok(locale)
    }
}

/// Middleware rendering problem+json responses in the request's locale.
///
/// [`Error`] responses are rendered in English and carry their unrendered
/// problem; this layer takes it off the response, resolves the locale (only
/// then), re-renders `title` and `detail` and sets `Content-Language`.
/// Other responses pass through untouched, so stacking it costs nothing:
/// [`compose`](crate::operations::compose) puts it on both sides of the
/// idempotency layer, and the server outermost for its fallbacks.
pub async fn localize(State(state): State<AppState>, mut request: Request, next: Next) -> Response {
    let slot = if let Some(slot) = request.extensions().get::<Slot>() {
        slot.clone()
    } else {
        let slot = Slot::default();
        request.extensions_mut().insert(slot.clone());
        slot
    };
    let inputs = Inputs::of(request.headers(), request.extensions());
    let mut response = next.run(request).await;
    let Some(problem) = response.extensions_mut().remove::<LocalizableProblem>() else {
        return response;
    };
    response
        .extensions_mut()
        .insert(crate::error::ProblemCode(problem.code()));
    let locale = if let Some(locale) = slot.0.get() {
        *locale
    } else {
        let locale = inputs.resolve_within_limit(&state).await;
        let _ = slot.0.set(locale);
        locale
    };
    problem.localize(locale, &mut response);
    response
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeSet,
        sync::atomic::{AtomicUsize, Ordering},
    };

    use axum::{Router, body::Body, http::StatusCode, routing::get};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    use super::*;
    use crate::{i18n::MESSAGES, testing};

    /// A port that answers `saved` for user `saver` and counts its calls.
    #[derive(Debug, Default)]
    struct FakePort {
        saved: Option<Locale>,
        fail: bool,
        calls: AtomicUsize,
        delay: Option<std::time::Duration>,
    }

    impl LocalePreference for FakePort {
        fn preferred<'a>(&'a self, _db: &'a PgPool, principal: &'a Principal) -> LocaleFuture<'a> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Box::pin(async move {
                if let Some(delay) = self.delay {
                    tokio::time::sleep(delay).await;
                }
                if self.fail {
                    return Err(Error::internal("database down"));
                }
                Ok(self.saved.filter(|_| principal.subject == "saver"))
            })
        }
    }

    fn principal(subject: &str, kind: PrincipalKind) -> Principal {
        Principal {
            subject: subject.to_owned(),
            kind,
            client_id: None,
            scopes: BTreeSet::new(),
            workspace: None,
            email: None,
            email_verified: false,
            name: None,
            expires_at: None,
            issued_at: None,
            auth_time: None,
            sign_in: None,
        }
    }

    fn app(port: Arc<FakePort>) -> Router {
        let state = testing::state(testing::unreachable_pool()).with_locales(port);
        Router::new()
            .route("/locale", get(|locale: Locale| async move { locale.tag() }))
            .route(
                "/conflict",
                get(|_: Locale| async {
                    Error::Conflict(MESSAGES.reason("idempotency_key_in_flight"))
                }),
            )
            .route("/missing", get(|| async { Error::NotFound }))
            .route_layer(axum::middleware::from_fn_with_state(
                state.clone(),
                localize,
            ))
            .with_state(state)
    }

    struct Reply {
        status: StatusCode,
        language: Option<String>,
        body: String,
    }

    async fn call(
        app: &Router,
        path: &str,
        accept: Option<&str>,
        principal: Option<Principal>,
    ) -> Reply {
        let mut request = axum::http::Request::get(path);
        if let Some(accept) = accept {
            request = request.header(header::ACCEPT_LANGUAGE, accept);
        }
        let mut request = request.body(Body::empty()).unwrap();
        if let Some(principal) = principal {
            request
                .extensions_mut()
                .insert(AuthenticatedPrincipal(principal));
        }
        let response = app.clone().oneshot(request).await.unwrap();
        Reply {
            status: response.status(),
            language: response
                .headers()
                .get(header::CONTENT_LANGUAGE)
                .map(|v| v.to_str().unwrap().to_owned()),
            body: String::from_utf8(
                response
                    .into_body()
                    .collect()
                    .await
                    .unwrap()
                    .to_bytes()
                    .to_vec(),
            )
            .unwrap(),
        }
    }

    fn json(reply: &Reply) -> serde_json::Value {
        serde_json::from_str(&reply.body).unwrap()
    }

    /// A saved locale that takes too long is not waited for: the problem is
    /// in the `Accept-Language` locale within the lookup limit
    /// (campbooks-cloud#265).
    #[tokio::test(start_paused = true)]
    async fn a_slow_saved_locale_falls_back_to_accept_language() {
        let port = Arc::new(FakePort {
            saved: Some(Locale::Fr),
            delay: Some(std::time::Duration::from_secs(10)),
            ..FakePort::default()
        });
        let app = app(port);
        let started = tokio::time::Instant::now();
        let reply = call(
            &app,
            "/missing",
            Some("pt-PT"),
            Some(principal("saver", PrincipalKind::User)),
        )
        .await;
        assert_eq!(reply.status, StatusCode::NOT_FOUND);
        assert_eq!(reply.language.as_deref(), Some("pt-PT"));
        assert!(started.elapsed() < std::time::Duration::from_secs(5));
    }

    #[tokio::test]
    async fn accept_language_then_english() {
        let app = app(Arc::default());
        for (accept, expected) in [
            (Some("pt-BR, en;q=0.5"), "pt-PT"),
            (Some("de-DE, fr;q=0.3"), "fr"),
            (Some("de"), "en"),
            (None, "en"),
        ] {
            assert_eq!(
                call(&app, "/locale", accept, None).await.body,
                expected,
                "{accept:?}"
            );
        }
    }

    #[tokio::test]
    async fn problems_render_in_the_request_locale() {
        let app = app(Arc::default());
        let reply = call(&app, "/missing", Some("es-MX"), None).await;
        assert_eq!(reply.status, StatusCode::NOT_FOUND);
        assert_eq!(reply.language.as_deref(), Some("es-ES"));
        let body = json(&reply);
        assert_eq!(body["code"], "not_found");
        assert_eq!(body["title"], "No encontrado");
        assert_eq!(body["type"], "about:blank");
        assert_eq!(
            body["detail"],
            MESSAGES.t(Locale::EsEs, "errors.not_found", &[])
        );

        let english = call(&app, "/missing", None, None).await;
        assert_eq!(english.language.as_deref(), Some("en"));
        assert_eq!(json(&english)["title"], "Not found");
    }

    #[tokio::test]
    async fn the_saved_locale_wins_and_is_read_once() {
        let port = Arc::new(FakePort {
            saved: Some(Locale::EsEs),
            ..FakePort::default()
        });
        let app = app(Arc::clone(&port));
        let saver = || Some(principal("saver", PrincipalKind::User));
        assert_eq!(
            call(&app, "/locale", Some("fr"), saver()).await.body,
            "es-ES"
        );

        // The handler's extractor and the middleware share one resolution.
        port.calls.store(0, Ordering::SeqCst);
        let reply = call(&app, "/conflict", Some("fr"), saver()).await;
        assert_eq!(reply.status, StatusCode::CONFLICT);
        assert_eq!(reply.language.as_deref(), Some("es-ES"));
        assert_eq!(json(&reply)["code"], "idempotency_key_in_flight");
        assert_eq!(json(&reply)["title"], "Conflicto");
        assert_eq!(port.calls.load(Ordering::SeqCst), 1);

        // A user who chose nothing, and a client, fall back to the header.
        let other = Some(principal("someone", PrincipalKind::User));
        assert_eq!(call(&app, "/locale", Some("fr"), other).await.body, "fr");
        port.calls.store(0, Ordering::SeqCst);
        let client = Some(principal("saver", PrincipalKind::Client));
        assert_eq!(call(&app, "/locale", Some("fr"), client).await.body, "fr");
        assert_eq!(
            port.calls.load(Ordering::SeqCst),
            0,
            "clients have no saved locale"
        );
    }

    #[tokio::test]
    async fn a_failing_port_falls_back_to_the_header() {
        let app = app(Arc::new(FakePort {
            fail: true,
            ..FakePort::default()
        }));
        let user = Some(principal("saver", PrincipalKind::User));
        assert_eq!(call(&app, "/locale", Some("fr-CA"), user).await.body, "fr");
    }

    #[tokio::test]
    async fn other_responses_are_untouched() {
        let app = app(Arc::default());
        let reply = call(&app, "/locale", Some("fr"), None).await;
        assert_eq!(reply.status, StatusCode::OK);
        assert_eq!(reply.language, None);
    }

    #[tokio::test]
    async fn a_bearer_token_names_the_user() {
        let idp = testing::TestIdp::start().await;
        let port = Arc::new(FakePort {
            saved: Some(Locale::PtPt),
            ..FakePort::default()
        });
        let state = idp
            .state(testing::unreachable_pool())
            .with_locales(Arc::clone(&port) as LocalePreferencePort);
        let app = Router::new()
            .route("/locale", get(|locale: Locale| async move { locale.tag() }))
            .with_state(state);
        let mut claims = idp.claims("");
        claims["sub"] = "saver".into();
        let request = axum::http::Request::get("/locale")
            .header(
                header::AUTHORIZATION,
                format!("Bearer {}", idp.token(&claims)),
            )
            .header(header::ACCEPT_LANGUAGE, "fr")
            .body(Body::empty())
            .unwrap();
        let response = app.clone().oneshot(request).await.unwrap();
        let body = response.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(&body[..], b"pt-PT");

        // An invalid token is no user: the header decides.
        let request = axum::http::Request::get("/locale")
            .header(header::AUTHORIZATION, "Bearer not-a-jwt")
            .header(header::ACCEPT_LANGUAGE, "fr")
            .body(Body::empty())
            .unwrap();
        let response = app.oneshot(request).await.unwrap();
        let body = response.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(&body[..], b"fr");
    }
}
