//! Checking an IMAP mailbox's address (campbooks-cloud#314), end to end
//! against the fake servers: the code sent through the mailbox's own SMTP
//! server, found in that connection's Inbox only, the address verified and
//! sending let through; never verified by a lying server or another
//! check's code; the message removed by its UID only, or left; the limits.

use axum::http::StatusCode;
use base64::Engine as _;
use campbooks_kernel::{
    clock::Clock as _,
    db::BeginFor as _,
    i18n::Locale,
    tenancy::{Role, WorkspaceId},
    testing,
};
use chrono::{TimeDelta, Utc};
use serde_json::{Value, json};
use sqlx::PgPool;

use super::{encoded_word, message};
use crate::{
    imap::{Security, Servers},
    testkit::{Caller, Kit},
    testkit_imap::{self, FakeServer, IMAP_HOST, SMTP_HOST, Script},
    testkit_imap_mailbox::Offers,
};

/// Ana's mailbox `ana@bakery.example`, signed in as `ana` (so its address
/// is not proven), connected: the check is sent once the jobs run.
struct Mailbox {
    kit: Kit,
    imap: FakeServer,
    smtp: FakeServer,
    ws: WorkspaceId,
    ana: Caller,
    connection: String,
    servers: Servers,
}

async fn mailbox(
    db: PgPool,
    offers: impl FnOnce(&mut Offers),
    smtp: impl FnOnce(&mut Script),
) -> Mailbox {
    let (kit, fakes) = testkit_imap::kit(db).await;
    let ws = WorkspaceId::new();
    let ana = kit.user_signed_in("ana", 5);
    kit.join(&ana, ws, Role::Member);
    let imap = FakeServer::imap(Security::Tls, Script::accepting("ana", "pw")).await;
    let smtp_server = FakeServer::smtp(Security::Starttls, Script::accepting("ana", "pw")).await;
    fakes.resolver.name(IMAP_HOST, &["127.0.0.1"]);
    fakes.resolver.name(SMTP_HOST, &["127.0.0.1"]);
    imap.script(|script| offers(&mut script.offers));
    // A Trash folder the stream reads (where MOVE takes a found check).
    imap.mailbox(|store| store.add_folder("Trash", &["\\Trash"]));
    let servers = Servers {
        imap: imap.at(IMAP_HOST),
        smtp: smtp_server.at(SMTP_HOST),
    };
    let connected = kit
        .post(
            &ana,
            &format!("/api/v1/w/{ws}/connections/imap"),
            json!({
                "address": "ana@bakery.example",
                "username": "ana",
                "password": "pw",
                "servers": &servers,
            }),
        )
        .await;
    assert_eq!(
        connected.status,
        StatusCode::CREATED,
        "{:?}",
        connected.json
    );
    assert_eq!(connected.json["address_verified"], false);
    assert_eq!(connected.json["address_check"], "checking");
    smtp_server.script(smtp);
    let connection = connected.json["id"].as_str().unwrap().to_owned();
    let m = Mailbox {
        kit,
        imap,
        smtp: smtp_server,
        ws,
        ana,
        connection,
        servers,
    };
    m.kit.run_jobs().await;
    m
}

impl Mailbox {
    /// The connection as the API shows it.
    async fn connection(&self) -> Value {
        let got = self
            .kit
            .get(
                &self.ana,
                &format!("/api/v1/w/{}/connections/{}", self.ws, self.connection),
            )
            .await;
        assert_eq!(got.status, StatusCode::OK, "{:?}", got.json);
        got.json
    }

    /// The check messages the SMTP server took, in order.
    fn checks(&self) -> Vec<Vec<u8>> {
        self.smtp.seen().delivered
    }

    /// The `Message-ID` of `raw`.
    fn id_of(raw: &[u8]) -> String {
        let text = String::from_utf8_lossy(raw);
        text.lines()
            .find_map(|line| line.strip_prefix("Message-ID: "))
            .unwrap()
            .trim()
            .to_owned()
    }

    /// Put `raw` in the Inbox, as the provider would deliver it.
    fn deliver(&self, raw: &[u8]) -> u32 {
        self.imap.deliver("INBOX", raw, self.kit.clock.now())
    }

    /// The stream's next run, a couple of minutes on.
    async fn run(&self) {
        self.kit.tick(TimeDelta::minutes(2)).await;
    }

    async fn ask_again(&self) -> (StatusCode, Value) {
        let asked = self
            .kit
            .post(
                &self.ana,
                &format!(
                    "/api/v1/w/{}/connections/{}/address-check",
                    self.ws, self.connection
                ),
                json!(null),
            )
            .await;
        (asked.status, asked.json)
    }
}

impl Mailbox {
    /// Why the check ended, if it did.
    async fn reason(&self) -> Option<String> {
        sqlx::query_scalar("SELECT reason FROM connectors.address_checks")
            .fetch_one(&self.kit.db)
            .await
            .unwrap()
    }

    /// Fail the check under way, as a run past its window would.
    async fn fail_check(&self) {
        sqlx::query("UPDATE connectors.address_checks SET state = 'failed'")
            .execute(&self.kit.db)
            .await
            .unwrap();
    }
}

/// A message in the Inbox that is not a check.
fn unrelated(id: &str) -> Vec<u8> {
    format!(
        "From: Bea <bea@example.com>\r\nTo: ana@bakery.example\r\nSubject: Order\r\n\
         Message-ID: <{id}>\r\nDate: Mon, 2 Nov 2026 10:00:00 +0000\r\n\r\nHello.\r\n"
    )
    .into_bytes()
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_code_found_in_the_inbox_verifies_the_address(db: PgPool) {
    let m = mailbox(db, |offers| offers.uidplus = true, |_| {}).await;
    // Sent once, from and to the address, through the mailbox's SMTP server.
    let checks = m.checks();
    assert_eq!(checks.len(), 1);
    let seen = m.smtp.seen();
    assert_eq!(seen.mail_froms, ["ana@bakery.example"]);
    assert_eq!(seen.rcpts, ["ana@bakery.example"]);
    assert!(!seen.mail_in_clear && !seen.password_in_clear);
    let raw = &checks[0];
    let text = String::from_utf8_lossy(raw);
    assert!(raw.is_ascii());
    assert!(text.contains("Auto-Submitted: auto-generated"), "{text}");
    let id = Mailbox::id_of(raw);
    assert!(
        id.starts_with("<cb-check-") && id.ends_with("@bakery.example>"),
        "{id}"
    );
    // The code is 128 bits, in no log, payload or row.
    let code = id
        .trim_start_matches("<cb-check-")
        .split('@')
        .next()
        .unwrap();
    assert_eq!(code.len(), 32);
    let stored: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM kernel.jobs WHERE payload::text LIKE '%' || $1 || '%'",
    )
    .bind(code)
    .fetch_one(&m.kit.db)
    .await
    .unwrap();
    assert_eq!(stored, 0);
    // Not delivered yet: still checking, the stream at its shortest interval.
    assert_eq!(m.connection().await["address_check"], "checking");
    let due: chrono::DateTime<Utc> =
        serde_json::from_value(m.kit.stream().await["due_at"].clone()).unwrap();
    assert!(due <= m.kit.clock.now() + TimeDelta::seconds(60), "{due}");

    let other = m.deliver(&unrelated("order-1@example.com"));
    let check = m.deliver(raw);
    m.run().await;

    let connection = m.connection().await;
    assert_eq!(connection["address_verified"], true);
    assert!(connection["address_check"].is_null());
    // Removed by its UID only (UIDPLUS); nothing else touched.
    let left = m.imap.mailbox(|store| store.uids("INBOX"));
    assert!(left.contains(&other) && !left.contains(&check), "{left:?}");
    let seen = m.imap.seen();
    assert!(
        seen.lines
            .iter()
            .any(|l| l == &format!("UID EXPUNGE {check}")),
        "{:?}",
        seen.lines
    );
    assert_eq!(seen.count("EXPUNGE"), 0, "never a plain EXPUNGE");
    assert_eq!(seen.count("CLOSE"), 0);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_verified_address_sends(db: PgPool) {
    let m = mailbox(db, |_| {}, |_| {}).await;
    let raw = m.checks().remove(0);
    m.deliver(&raw);
    m.run().await;
    let verified: bool = sqlx::query_scalar("SELECT address_verified FROM connectors.connections")
        .fetch_one(&m.kit.db)
        .await
        .unwrap();
    assert!(verified, "what the send job reads to let a send through");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_code_that_never_comes_fails_the_check(db: PgPool) {
    let m = mailbox(db, |_| {}, |_| {}).await;
    m.run().await;
    assert_eq!(m.connection().await["address_check"], "checking");
    m.kit.tick(TimeDelta::minutes(16)).await;
    let connection = m.connection().await;
    assert_eq!(connection["address_verified"], false);
    assert_eq!(connection["address_check"], "failed");
    let reason: Option<String> = sqlx::query_scalar("SELECT reason FROM connectors.address_checks")
        .fetch_one(&m.kit.db)
        .await
        .unwrap();
    assert_eq!(reason.as_deref(), Some("not_found"));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_lying_server_verifies_nothing(db: PgPool) {
    // Every search answers with every message; none carries the code.
    let m = mailbox(db, |offers| offers.lying_search = true, |_| {}).await;
    m.deliver(&unrelated("order-1@example.com"));
    m.deliver(&unrelated(
        "cb-check-00000000000000000000000000000000@bakery.example",
    ));
    m.run().await;
    m.run().await;
    let connection = m.connection().await;
    assert_eq!(connection["address_verified"], false);
    assert_eq!(connection["address_check"], "checking");
    let searches: i32 = sqlx::query_scalar("SELECT searches FROM connectors.address_checks")
        .fetch_one(&m.kit.db)
        .await
        .unwrap();
    assert!(searches >= 2, "{searches}");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn only_the_code_of_this_check_counts(db: PgPool) {
    let m = mailbox(db, |_| {}, |_| {}).await;
    let first = m.checks().remove(0);
    // The first check ends unfound; it is asked for again.
    m.kit.tick(TimeDelta::minutes(16)).await;
    assert_eq!(m.connection().await["address_check"], "failed");
    let (status, json) = m.ask_again().await;
    assert_eq!(status, StatusCode::ACCEPTED, "{json:?}");
    assert_eq!(json["address_check"], "checking");
    m.kit.run_jobs().await;
    let second = m.checks().remove(1);
    assert_ne!(Mailbox::id_of(&first), Mailbox::id_of(&second));

    // The earlier code arrives late: it counts for nothing.
    m.deliver(&first);
    m.run().await;
    assert_eq!(m.connection().await["address_verified"], false);
    m.deliver(&second);
    m.run().await;
    assert_eq!(m.connection().await["address_verified"], true);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn another_message_marked_deleted_keeps_the_check_where_it_is(db: PgPool) {
    let m = mailbox(db, |offers| offers.uidplus = true, |_| {}).await;
    let other = m.deliver(&unrelated("order-1@example.com"));
    m.imap.mailbox(|store| store.set_deleted("INBOX", other));
    let check = m.deliver(&m.checks().remove(0));
    m.run().await;
    assert_eq!(m.connection().await["address_verified"], true);
    let left = m.imap.mailbox(|store| store.uids("INBOX"));
    assert!(left.contains(&other) && left.contains(&check), "{left:?}");
    assert_eq!(m.imap.seen().count("UID EXPUNGE"), 0);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn without_uidplus_or_move_the_check_is_left(db: PgPool) {
    let m = mailbox(db, |_| {}, |_| {}).await;
    let check = m.deliver(&m.checks().remove(0));
    m.run().await;
    assert_eq!(m.connection().await["address_verified"], true);
    assert!(m.imap.mailbox(|store| store.uids("INBOX")).contains(&check));
    let seen = m.imap.seen();
    assert_eq!(seen.count("UID STORE") + seen.count("UID MOVE"), 0);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn with_move_the_check_goes_to_trash(db: PgPool) {
    let m = mailbox(
        db,
        |offers| {
            offers.moves = true;
        },
        |_| {},
    )
    .await;
    let check = m.deliver(&m.checks().remove(0));
    m.run().await;
    assert_eq!(m.connection().await["address_verified"], true);
    assert!(!m.imap.mailbox(|store| store.uids("INBOX")).contains(&check));
    assert_eq!(m.imap.mailbox(|store| store.uids("Trash")).len(), 1);
    assert!(
        m.imap
            .seen()
            .lines
            .iter()
            .any(|l| l.starts_with(&format!("UID MOVE {check} "))),
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_server_that_will_not_send_as_the_address_fails_the_check(db: PgPool) {
    let m = mailbox(
        db,
        |_| {},
        |smtp| smtp.smtp.refuse_rcpt = Some("ana@bakery.example".to_owned()),
    )
    .await;
    assert!(m.checks().is_empty());
    assert_eq!(m.connection().await["address_check"], "failed");
    let reason: Option<String> = sqlx::query_scalar("SELECT reason FROM connectors.address_checks")
        .fetch_one(&m.kit.db)
        .await
        .unwrap();
    assert_eq!(reason.as_deref(), Some("address_refused"));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_address_is_checked_at_most_five_times_a_day(db: PgPool) {
    let m = mailbox(db, |_| {}, |_| {}).await;
    // The connect's check, then four more.
    for _ in 0..4 {
        sqlx::query("UPDATE connectors.address_checks SET state = 'failed'")
            .execute(&m.kit.db)
            .await
            .unwrap();
        let (status, json) = m.ask_again().await;
        assert_eq!(status, StatusCode::ACCEPTED, "{json:?}");
    }
    sqlx::query("UPDATE connectors.address_checks SET state = 'failed'")
        .execute(&m.kit.db)
        .await
        .unwrap();
    let (status, json) = m.ask_again().await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "{json:?}");
    assert_eq!(json["code"], "rate_limited");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_check_under_way_is_answered_as_it_is(db: PgPool) {
    let m = mailbox(db, |_| {}, |_| {}).await;
    let (status, json) = m.ask_again().await;
    assert_eq!(status, StatusCode::ACCEPTED);
    assert_eq!(json["address_check"], "checking");
    m.kit.run_jobs().await;
    assert_eq!(m.checks().len(), 1, "no second message");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_verified_address_needs_no_check(db: PgPool) {
    let (kit, fakes) = testkit_imap::kit(db).await;
    let ws = WorkspaceId::new();
    let ana = kit.user_signed_in("ana", 5);
    kit.join(&ana, ws, Role::Member);
    let (_imap, smtp, servers) = testkit_imap::mailbox(&fakes, "ana@bakery.example", "pw").await;
    let connected = kit
        .post(
            &ana,
            &format!("/api/v1/w/{ws}/connections/imap"),
            json!({"address": "ana@bakery.example", "password": "pw", "servers": servers}),
        )
        .await;
    assert_eq!(connected.json["address_verified"], true);
    assert!(connected.json["address_check"].is_null());
    kit.run_jobs().await;
    assert!(!smtp.seen().mail_from, "no check sent");
    let id = connected.json["id"].as_str().unwrap();
    let asked = kit
        .post(
            &ana,
            &format!("/api/v1/w/{ws}/connections/{id}/address-check"),
            json!(null),
        )
        .await;
    assert_eq!(asked.status, StatusCode::CONFLICT);
    assert_eq!(asked.json["code"], "address_check_not_needed");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn address_checks_are_isolated_by_workspace(db: PgPool) {
    let m = mailbox(db, |_| {}, |_| {}).await;
    let app = testing::app_role_pool(&m.kit.db).await;
    let count = "SELECT count(*) FROM connectors.address_checks";
    let mut tx = app.begin_for(m.ws).await.unwrap();
    let own: i64 = sqlx::query_scalar(count).fetch_one(&mut *tx).await.unwrap();
    assert_eq!(own, 1);
    tx.rollback().await.unwrap();
    let mut tx = app.begin_for(WorkspaceId::new()).await.unwrap();
    let other: i64 = sqlx::query_scalar(count).fetch_one(&mut *tx).await.unwrap();
    assert_eq!(other, 0);
    let written = sqlx::query("UPDATE connectors.address_checks SET state = 'verified'")
        .execute(&mut *tx)
        .await
        .unwrap();
    assert_eq!(written.rows_affected(), 0);
    tx.rollback().await.unwrap();
    let mut tx = app.begin().await.unwrap();
    let none: i64 = sqlx::query_scalar(count).fetch_one(&mut *tx).await.unwrap();
    assert_eq!(none, 0);
    tx.rollback().await.unwrap();
}

#[test]
fn the_message_is_plain_ascii_text_in_the_members_language() {
    let raw = message(
        Locale::PtPt,
        "ana@bakery.example",
        "<cb-check-0123@bakery.example>",
        Utc::now(),
    );
    assert!(raw.is_ascii());
    let text = String::from_utf8(raw).unwrap();
    let (head, body) = text.split_once("\r\n\r\n").unwrap();
    for header in [
        "From: <ana@bakery.example>",
        "To: <ana@bakery.example>",
        "Message-ID: <cb-check-0123@bakery.example>",
        "Content-Type: text/plain; charset=utf-8",
        "Auto-Submitted: auto-generated",
    ] {
        assert!(head.lines().any(|l| l == header), "{header}: {head}");
    }
    assert!(head.contains("Subject: =?UTF-8?B?"), "{head}");
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(body.replace("\r\n", ""))
        .unwrap();
    let decoded = String::from_utf8(decoded).unwrap();
    assert!(decoded.contains("ana@bakery.example"), "{decoded}");
    assert!(decoded.contains("Campbooks"), "{decoded}");
}

#[test]
fn encoded_words_never_carry_a_line_break() {
    assert_eq!(encoded_word("Plain subject"), "Plain subject");
    assert_eq!(encoded_word("Line\r\nBcc: x"), "Line  Bcc: x");
    let long = encoded_word(&"verificação ".repeat(10));
    for word in long.split("\r\n ") {
        assert!(
            word.starts_with("=?UTF-8?B?") && word.ends_with("?="),
            "{word}"
        );
        assert!(word.len() <= 75, "{word}");
    }
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_address_changed_once_the_checks_are_spent_is_never_verified(db: PgPool) {
    let m = mailbox(db, |_| {}, |_| {}).await;
    // The day's checks spent: the connect's, then four more, the last sent
    // to the address the member owns, its code read by them.
    for _ in 0..4 {
        m.fail_check().await;
        let (status, json) = m.ask_again().await;
        assert_eq!(status, StatusCode::ACCEPTED, "{json:?}");
    }
    m.kit.run_jobs().await;
    let fifth = m.checks().pop().unwrap();

    // The same connection, connected again under someone else's address,
    // with no check left for it.
    let again = m
        .kit
        .post(
            &m.ana,
            &format!("/api/v1/w/{}/connections/imap", m.ws),
            json!({
                "address": "victim@victim.example",
                "username": "ana",
                "password": "pw",
                "servers": &m.servers,
                "connection": m.connection,
            }),
        )
        .await;
    assert_eq!(again.status, StatusCode::CREATED, "{:?}", again.json);
    assert_eq!(again.json["address"], "victim@victim.example");
    assert_eq!(again.json["address_check"], "failed");
    assert_eq!(m.reason().await.as_deref(), Some("address_changed"));

    // The fifth code, filed in the Inbox by the member, verifies nothing.
    m.deliver(&fifth);
    m.run().await;
    let connection = m.connection().await;
    assert_eq!(connection["address"], "victim@victim.example");
    assert_eq!(connection["address_verified"], false);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_code_for_one_address_never_verifies_another(db: PgPool) {
    let m = mailbox(db, |_| {}, |_| {}).await;
    let raw = m.checks().remove(0);
    // The connection's address changed under the check (by any path).
    sqlx::query("UPDATE connectors.connections SET address = 'billing@bakery.example'")
        .execute(&m.kit.db)
        .await
        .unwrap();
    m.deliver(&raw);
    m.run().await;
    assert_eq!(m.connection().await["address_verified"], false);
    let verified: bool = sqlx::query_scalar("SELECT address_verified FROM connectors.connections")
        .fetch_one(&m.kit.db)
        .await
        .unwrap();
    assert!(!verified);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_check_past_its_window_is_over_even_if_no_run_looked(db: PgPool) {
    let m = mailbox(db, |_| {}, |_| {}).await;
    let raw = m.checks().remove(0);
    m.deliver(&raw);
    // No run for 16 minutes (a paused or failing stream), then one.
    m.kit.clock.advance(TimeDelta::minutes(16));
    m.run().await;
    let connection = m.connection().await;
    assert_eq!(connection["address_verified"], false);
    assert_eq!(connection["address_check"], "failed");
    assert_eq!(m.reason().await.as_deref(), Some("not_found"));
}

#[test]
fn a_code_is_made_for_one_address() {
    use campbooks_kernel::crypto::{SecretKey, Signer};

    use crate::domain::{address_checks::code, model::ConnectionId};

    let signer = Signer::new(&SecretKey::from_bytes([1; 32]));
    let (connection, nonce) = (ConnectionId::new(), uuid::Uuid::now_v7());
    assert_ne!(
        code(&signer, connection, nonce, "ana@bakery.example"),
        code(&signer, connection, nonce, "billing@bakery.example")
    );
    assert_eq!(
        code(&signer, connection, nonce, " Ana@Bakery.example "),
        code(&signer, connection, nonce, "ana@bakery.example")
    );
    assert_eq!(
        code(&signer, connection, nonce, "ana@bakery.example").len(),
        32
    );
}
