//! Mail-like content fed through the error paths that used to quote it: a
//! provider's error body, a Postgres error's message and `DETAIL`. Nothing of
//! it may reach a log line or a job's `last_error` (campbooks-cloud#117).

// Helpers outside `#[test]` fns are not covered by clippy's test exemptions.
#![allow(clippy::unwrap_used)]

use std::{
    io::Write,
    sync::{Arc, Mutex, PoisonError},
    time::Duration,
};

use axum::response::IntoResponse as _;
use campbooks_ai::{
    AiConfig, Gateway, Protocol, ProviderConfig, RetryPolicy, Role,
    decide::{
        signals::email::{Envelope, Party, triage_questions},
        strategy::{Cascade, Model, Stage, Strategy as _},
    },
};
use campbooks_kernel::JobError;
use sqlx::PgPool;
use tracing_subscriber::{fmt::MakeWriter, prelude::*};
use wiremock::{Mock, MockServer, ResponseTemplate, matchers::method};

const ADDRESS: &str = "maya@northwind-studio.example";
const SUBJECT: &str = "Q4 restructuring plan";

/// Everything the fmt layer writes, at every level.
#[derive(Clone, Default)]
struct Logs(Arc<Mutex<Vec<u8>>>);

impl Logs {
    fn capture() -> (Self, tracing::subscriber::DefaultGuard) {
        let logs = Self::default();
        let subscriber = tracing_subscriber::registry().with(
            tracing_subscriber::fmt::layer()
                .with_ansi(false)
                .with_writer(logs.clone())
                .with_filter(tracing_subscriber::filter::LevelFilter::TRACE),
        );
        let guard = tracing::subscriber::set_default(subscriber);
        (logs, guard)
    }

    fn text(&self) -> String {
        String::from_utf8(
            self.0
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .clone(),
        )
        .unwrap()
    }
}

impl Write for Logs {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> MakeWriter<'a> for Logs {
    type Writer = Self;

    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

fn assert_clean(what: &str, text: &str) {
    for content in [ADDRESS, "northwind", SUBJECT, "restructuring"] {
        assert!(
            !text.contains(content),
            "{what} carries `{content}`:\n{text}"
        );
    }
}

#[tokio::test]
async fn a_provider_echoing_the_mail_puts_none_of_it_in_the_logs() {
    // The provider refuses and quotes what it was sent.
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(400)
                .set_body_string(format!("bad request: Subject: {SUBJECT} From: {ADDRESS}")),
        )
        .mount(&server)
        .await;
    let gateway = Gateway::new(AiConfig {
        roles: [(
            Role::Decide,
            ProviderConfig::new(format!("{}/v1", server.uri()).parse().unwrap(), "laya")
                .with_protocol(Protocol::Laya),
        )]
        .into(),
        ..AiConfig::default()
    })
    .unwrap()
    .with_retry(RetryPolicy {
        attempts: 1,
        base_delay: Duration::ZERO,
        max_delay: Duration::ZERO,
    });
    let questions = triage_questions();
    let cascade = Cascade::new(vec![Stage {
        strategy: Arc::new(Model::new("decide", gateway, "test.redaction")),
        thresholds: questions.iter().map(|q| (q.id.clone(), 0.0)).collect(),
    }]);
    let envelope = Envelope {
        from: Party {
            name: "Maya".into(),
            address: ADDRESS.into(),
        },
        subject: SUBJECT.into(),
        body: format!("Hi, the {SUBJECT} is attached. Maya"),
        ..Envelope::default()
    };

    let (logs, _guard) = Logs::capture();
    let decided = cascade.decide(&envelope, &questions).await.unwrap();
    assert!(
        decided.answers.iter().all(Option::is_none),
        "the stage abstained"
    );

    let text = logs.text();
    assert!(text.contains("decide stage failed"), "{text}");
    assert!(text.contains("AI provider returned 400"), "{text}");
    assert_clean("the log", &text);
}

#[sqlx::test(migrations = false)]
async fn a_database_error_quoting_a_row_puts_none_of_it_in_the_logs(db: PgPool) {
    sqlx::query("CREATE TABLE contacts (email text CONSTRAINT contacts_email_key UNIQUE)")
        .execute(&db)
        .await
        .unwrap();
    let insert = || {
        sqlx::query("INSERT INTO contacts (email) VALUES ($1)")
            .bind(ADDRESS)
            .execute(&db)
    };
    insert().await.unwrap();
    // `Key (email)=(maya@…) already exists` is in the error's DETAIL.
    let duplicate = insert().await.unwrap_err();
    assert!(
        format!("{duplicate:?}").contains(ADDRESS),
        "the raw error quotes it"
    );
    let unreadable = sqlx::query("SELECT $1::text::int")
        .bind(SUBJECT)
        .fetch_one(&db)
        .await
        .unwrap_err();
    assert!(
        unreadable.to_string().contains(SUBJECT),
        "the raw error quotes it"
    );

    // As a job's outcome (`kernel.jobs.last_error` is this message).
    let stored = JobError::from(
        sqlx::query("SELECT $1::text::int")
            .bind(SUBJECT)
            .fetch_one(&db)
            .await
            .unwrap_err(),
    )
    .to_string();
    assert_eq!(stored, "database error 22P02");

    // As a request's internal error.
    let (logs, _guard) = Logs::capture();
    for error in [duplicate, unreadable] {
        let response = campbooks_kernel::Error::from(error).into_response();
        assert_eq!(response.status(), 500);
    }
    let text = logs.text();
    assert!(
        text.contains("database error 23505 on constraint contacts_email_key in table contacts"),
        "{text}"
    );
    assert!(text.contains("database error 22P02"), "{text}");
    assert_clean("the log", &text);
}
