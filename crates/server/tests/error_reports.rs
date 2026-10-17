//! Error reports carry ids and error kinds, never the mail, addresses,
//! tokens or requests they happened around (`campbooks_server::error_reports`).
//! Its own test binary: it installs the process-wide panic hook.

// Helpers outside `#[test]` fns are not covered by clippy's test exemptions.
#![allow(clippy::unwrap_used)]

use std::{borrow::Cow, sync::Arc};

use campbooks_kernel::config::TelemetryConfig;
use campbooks_server::{
    error_reports::{self, ErrorReportLayer},
    telemetry,
};
use sentry::{
    Client, Hub, Scope,
    protocol::{Breadcrumb, Context, Event, Exception, Map, Request, User},
    test::TestTransport,
};
use tracing_subscriber::prelude::*;

const ADDRESS: &str = "maya@northwind-studio.example";
const SUBJECT: &str = "Q4 restructuring plan";
const BODY: &str = "Please find the board pack attached";
const ATTACHMENT: &str = "salaries-2026.xlsx";
const ACCESS_TOKEN: &str = "ya29.a0AfH6SMBx3examplelongaccesstokenvalue1234567890abcdef";
/// A made-up token shaped like a JWT, assembled so the source holds no
/// token-like literal (gitleaks).
const JWT: &str = concat!(
    "eyJhbGciOiJSUzI1NiJ9",
    ".",
    "eyJzdWIiOiJtYXlhIiwiZW1haWwiOiJtYXlhQG5vcnRod2luZCJ9",
    ".",
    "c2lnbmF0dXJlc2lnbmF0dXJl"
);
const COOKIE: &str = "__Host-campbooks_session=s3ss10n-c00k1e-v4lue";
const WORKSPACE: &str = "0192a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a5b";
const REQUEST_ID: &str = "0192a1b2-0000-7e5f-8a9b-0c1d2e3f4a5c";
const TRACE_ID: &str = "4bf92f3577b34da6a3ce929d0e0e4736";
const JOB_ID: &str = "0192a1b2-1111-7e5f-8a9b-0c1d2e3f4a5d";

/// Every planted secret or piece of content: none may appear in a report.
const LEAKS: &[&str] = &[
    ADDRESS,
    "northwind-studio",
    SUBJECT,
    BODY,
    ATTACHMENT,
    ACCESS_TOKEN,
    JWT,
    "s3ss10n-c00k1e-v4lue",
    "campbooks-host.internal",
    "203.0.113.7",
];

fn assert_clean(report: &str) {
    for leak in LEAKS {
        assert!(!report.contains(leak), "{leak} leaked into {report}");
    }
}

fn config() -> TelemetryConfig {
    TelemetryConfig {
        error_dsn: Some("https://public@errors.example.com/1".to_owned().into()),
        environment: Some("staging".to_owned()),
        ..TelemetryConfig::default()
    }
}

/// An event as an integration or a careless caller could fill it: every
/// field that can hold personal data holds some.
fn loaded_event() -> Event<'static> {
    let mut tags = Map::new();
    tags.insert("request_id".to_owned(), REQUEST_ID.to_owned());
    tags.insert("workspace.id".to_owned(), WORKSPACE.to_owned());
    tags.insert(
        "http.route".to_owned(),
        "/api/v1/w/{workspace_id}/messages".to_owned(),
    );
    tags.insert("user.email".to_owned(), ADDRESS.to_owned());
    tags.insert("subject".to_owned(), SUBJECT.to_owned());
    // An allowed key holding data is dropped too.
    tags.insert("kind".to_owned(), ADDRESS.to_owned());
    tags.insert("job_id".to_owned(), ACCESS_TOKEN.to_owned());
    let mut headers = Map::new();
    headers.insert("Authorization".to_owned(), format!("Bearer {JWT}"));
    headers.insert("Cookie".to_owned(), COOKIE.to_owned());
    let mut extra = Map::new();
    extra.insert("body".to_owned(), BODY.into());
    let mut contexts = Map::new();
    let mut fields = Map::new();
    fields.insert("attachment".to_owned(), ATTACHMENT.into());
    fields.insert("token".to_owned(), ACCESS_TOKEN.into());
    contexts.insert("Rust Tracing Fields".to_owned(), Context::Other(fields));
    Event {
        message: Some(format!("could not send \"{SUBJECT}\" to {ADDRESS}")),
        logger: Some("campbooks_connectors::mail".to_owned()),
        culprit: Some(format!("{ADDRESS} sent {SUBJECT}")),
        transaction: Some("POST /api/v1/w/{workspace_id}/messages".to_owned()),
        fingerprint: Cow::Owned(vec![Cow::Borrowed("internal error"), Cow::Owned(ADDRESS.to_owned())]),
        server_name: Some(Cow::Borrowed("campbooks-host.internal")),
        user: Some(User {
            id: Some(ADDRESS.to_owned()),
            email: Some(ADDRESS.to_owned()),
            ip_address: Some("203.0.113.7".parse().unwrap()),
            username: Some(ADDRESS.to_owned()),
            ..User::default()
        }),
        request: Some(Request {
            url: Some(format!("https://app.example.com/api/v1/search?q={SUBJECT}&token={ACCESS_TOKEN}").parse().unwrap()),
            method: Some("POST".to_owned()),
            data: Some(format!("{{\"to\":\"{ADDRESS}\",\"body\":\"{BODY}\"}}")),
            query_string: Some(format!("q={SUBJECT}")),
            cookies: Some(COOKIE.to_owned()),
            headers,
            ..Request::default()
        }),
        breadcrumbs: vec![Breadcrumb {
            message: Some(format!("opened {ATTACHMENT} from {ADDRESS}")),
            ..Breadcrumb::default()
        }]
        .into(),
        exception: vec![Exception {
            ty: "internal error".to_owned(),
            value: Some(format!(
                "provider answered 401 for https://gmail.googleapis.com/gmail/v1/users/{ADDRESS}/messages?access_token={ACCESS_TOKEN}: Bearer {JWT}"
            )),
            module: Some("campbooks_connectors::gmail".to_owned()),
            ..Exception::default()
        }]
        .into(),
        tags,
        extra,
        contexts,
        ..Event::default()
    }
}

#[test]
fn the_scrubber_keeps_ids_and_error_kinds_only() {
    let scrubbed = error_reports::scrub(loaded_event());
    let json = serde_json::to_string(&scrubbed).unwrap();
    assert_clean(&json);

    assert!(scrubbed.user.is_none());
    assert!(scrubbed.request.is_none());
    assert!(scrubbed.server_name.is_none());
    assert!(scrubbed.breadcrumbs.values.is_empty());
    assert!(scrubbed.extra.is_empty());
    assert!(scrubbed.contexts.is_empty());
    assert!(scrubbed.culprit.is_none());
    assert_eq!(
        scrubbed.fingerprint.as_ref(),
        [Cow::Borrowed("internal error"), Cow::Borrowed("[address]")]
    );
    let tags: Vec<&str> = scrubbed.tags.keys().map(String::as_str).collect();
    assert_eq!(tags, ["http.route", "request_id", "workspace.id"]);
    assert_eq!(scrubbed.tags["workspace.id"], WORKSPACE);
    let exception = &scrubbed.exception.values[0];
    assert_eq!(exception.ty, "internal error");
    let value = exception.value.as_deref().unwrap();
    // The kind of failure and where it went stay readable.
    assert!(value.contains("provider answered 401"), "{value}");
    assert!(value.contains("https://gmail.googleapis.com"), "{value}");
    // "POST /api/…" has a space: not an id, dropped.
    assert!(scrubbed.transaction.is_none());
}

#[test]
fn reports_from_requests_jobs_and_panics_hold_no_personal_data() {
    let transport = TestTransport::new();
    let mut options = error_reports::client_options(&config(), "all", None)
        .unwrap()
        .unwrap();
    options.transport = Some(Arc::new(Arc::clone(&transport)));
    let hub = Arc::new(Hub::new(
        Some(Arc::new(Client::from(options))),
        Arc::new(Scope::default()),
    ));
    telemetry::install_panic_hook();
    let subscriber = tracing_subscriber::registry().with(ErrorReportLayer);

    Hub::run(hub, || {
        tracing::subscriber::with_default(subscriber, || {
            let request = tracing::info_span!(
                "http",
                request_id = REQUEST_ID,
                trace_id = TRACE_ID,
                http.route = "/api/v1/w/{workspace_id}/messages",
                http.request.method = "POST",
                workspace.id = tracing::field::Empty,
                // Not allowlisted: never read.
                user.email = ADDRESS,
                q = SUBJECT,
            );
            request.record("workspace.id", WORKSPACE);
            request.in_scope(|| {
                // What an internal error whose source quoted a row looks like.
                tracing::error!(
                    error = %format!("database said Key (email)=({ADDRESS}) exists; token {ACCESS_TOKEN}"),
                    subject = SUBJECT,
                    authorization = %format!("Bearer {JWT}"),
                    "internal error"
                );
                // Below ERROR: no report.
                tracing::warn!(subject = SUBJECT, "slow");
            });

            let job = tracing::info_span!(
                "job",
                job.id = JOB_ID,
                job.kind = "connectors.sync_stream",
                job.queue = "default",
                job.attempt = 3_u32,
                workspace.id = WORKSPACE,
            );
            job.in_scope(|| {
                tracing::error!(error = %format!("\"{SUBJECT}\" from {ADDRESS}: {BODY}"), "job failed for good");
                let caught = std::panic::catch_unwind(|| {
                    let query = format!("older_than:5é {ATTACHMENT} for {ADDRESS}");
                    let _ = &query[..13];
                });
                assert!(caught.is_err());
            });
        });
        Hub::current().client().unwrap().flush(None);
    });
    let _ = std::panic::take_hook();

    let events = transport.fetch_and_clear_events();
    assert_eq!(events.len(), 3, "{events:#?}");
    for event in &events {
        let json = serde_json::to_string(event).unwrap();
        assert_clean(&json);
        assert_eq!(event.environment.as_deref(), Some("staging"));
        assert!(event.release.is_some());
        assert_eq!(event.tags["campbooks.role"], "all");
        assert!(event.user.is_none() && event.request.is_none());
        assert!(event.breadcrumbs.values.is_empty());
    }

    let internal = &events[0];
    assert_eq!(internal.exception.values[0].ty, "internal error");
    assert_eq!(internal.tags["request_id"], REQUEST_ID);
    assert_eq!(internal.tags["trace_id"], TRACE_ID);
    assert_eq!(internal.tags["workspace.id"], WORKSPACE);
    assert_eq!(
        internal.tags["http.route"],
        "/api/v1/w/{workspace_id}/messages"
    );
    // Its kind, not the row it quoted.
    assert_eq!(
        internal.exception.values[0].value.as_deref(),
        Some("database said Key")
    );
    assert!(internal.tags["location"].starts_with("crates/server/tests/error_reports.rs:"));

    let failed = &events[1];
    assert_eq!(failed.exception.values[0].ty, "job failed for good");
    assert_eq!(failed.tags["job.id"], JOB_ID);
    assert_eq!(failed.tags["job.kind"], "connectors.sync_stream");
    assert_eq!(failed.tags["job.attempt"], "3");
    assert_eq!(failed.culprit.as_deref(), Some("connectors.sync_stream"));
    assert_eq!(
        internal.culprit.as_deref(),
        Some("/api/v1/w/{workspace_id}/messages")
    );

    let panic = &events[2];
    assert_eq!(panic.exception.values[0].ty, "panic");
    let value = panic.exception.values[0].value.as_deref().unwrap();
    // Where it happened, never its message.
    assert!(
        value.starts_with("panicked at crates/server/tests/error_reports.rs:"),
        "{value}"
    );
    assert_eq!(panic.tags["job.kind"], "connectors.sync_stream");
}
