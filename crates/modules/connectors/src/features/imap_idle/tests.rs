//! IMAP push with IDLE (campbooks-cloud#318), end to end against the fake
//! server: a change heard in IDLE asks for one run, leases give a
//! connection one holder, polling stays where IDLE cannot, and a hostile
//! server, a guard that now refuses, or a changed password end the session
//! as they should. Time is the pool's: its windows are set short here
//! instead of waiting them out.

use std::time::Duration;

use axum::http::StatusCode;
use campbooks_kernel::{
    clock::Clock as _,
    db::BeginFor as _,
    tenancy::{Role, WorkspaceId},
    testing,
};
use chrono::{DateTime, TimeDelta, Utc};
use sqlx::PgPool;
use tokio::time::Instant;
use uuid::Uuid;

use super::{Held, IdlePool, Timing, lease};
use crate::{
    config::ImapConfig,
    testkit::{Caller, Kit},
    testkit_imap::{self, FakeServer, Fakes, IMAP_HOST},
};

/// A member's IMAP mailbox at the fake, connected and read once.
struct Mailbox {
    kit: Kit,
    imap: FakeServer,
    ws: WorkspaceId,
    fakes: Fakes,
    ana: Caller,
    smtp: FakeServer,
}

/// A message from Bea of the bakery.
fn from_bea(id: &str, subject: &str) -> Vec<u8> {
    format!(
        "From: Bea <bea@bakery.example>\r\nTo: Ana <ana@bakery.example>\r\n\
         Subject: {subject}\r\nDate: Mon, 5 Oct 2026 10:00:00 +0000\r\n\
         Message-ID: <{id}>\r\n\r\nTwo sourdough loaves for Friday, please.\r\n"
    )
    .into_bytes()
}

/// Connect Ana's mailbox at the fake in `ws`.
async fn connect(kit: &Kit, ana: &Caller, ws: WorkspaceId, imap: &FakeServer, smtp: &FakeServer) {
    let servers = crate::imap::Servers {
        imap: imap.at(IMAP_HOST),
        smtp: smtp.at(testkit_imap::SMTP_HOST),
    };
    let connected = kit
        .post(
            ana,
            &format!("/api/v1/w/{ws}/connections/imap"),
            serde_json::json!({
                "address": "ana@bakery.example",
                "password": "pw",
                "servers": servers,
            }),
        )
        .await;
    assert_eq!(
        connected.status,
        StatusCode::CREATED,
        "{:?}",
        connected.json
    );
}

/// Ana's mailbox, IMAP and IDLE on (`config` changes the rest), its server
/// offering IDLE when `idle`, connected and read once.
async fn mailbox(db: PgPool, idle: bool, config: impl FnOnce(&mut ImapConfig)) -> Mailbox {
    let (kit, fakes) = testkit_imap::kit_with(db, |c| {
        c.idle_enabled = true;
        config(c);
    })
    .await;
    let ws = WorkspaceId::new();
    let ana = kit.user_signed_in("ana", 5);
    kit.join(&ana, ws, Role::Member);
    let (imap, smtp, _) = testkit_imap::mailbox(&fakes, "ana@bakery.example", "pw").await;
    imap.script(|script| script.offers.idle = idle);
    imap.deliver(
        "INBOX",
        &from_bea("hello@bakery.example", "Hello"),
        kit.clock.now() - TimeDelta::hours(1),
    );
    connect(&kit, &ana, ws, &imap, &smtp).await;
    kit.run_jobs().await;
    Mailbox {
        kit,
        imap,
        ws,
        fakes,
        ana,
        smtp,
    }
}

/// Windows short enough for a test: a reconnect after 10 ms, given up after
/// three failures; a debounce longer than any test.
fn timing() -> Timing {
    Timing {
        refresh: Duration::from_secs(60),
        debounce: Duration::from_secs(600),
        renew: Duration::from_secs(60),
        lease: Duration::from_secs(180),
        backoff: Duration::from_millis(10),
        backoff_max: Duration::from_millis(40),
        give_up_after: 3,
        cooldown: Duration::from_secs(1_800),
    }
}

impl Mailbox {
    fn pool(&self, timing: Timing) -> IdlePool {
        IdlePool::with_timing(self.kit.deps.clone(), self.kit.state.clone(), timing)
    }

    /// A pool holding this mailbox in IDLE, once its session is in IDLE.
    async fn idling(&self, timing: Timing) -> (IdlePool, Held) {
        let pool = self.pool(timing);
        let mut held = IdlePool::held();
        pool.tick(&mut held).await;
        assert_eq!(held.connections().len(), 1, "the lease is claimed");
        until("the session to be in IDLE", async || {
            self.imap.seen().idles >= 1
        })
        .await;
        (pool, held)
    }

    async fn count(&self, query: &'static str) -> i64 {
        sqlx::query_scalar(query)
            .fetch_one(&self.kit.db)
            .await
            .unwrap()
    }

    /// The lease rows: holder, and whether live.
    async fn leases(&self) -> Vec<(Option<Uuid>, bool)> {
        sqlx::query_as(
            "SELECT holder, coalesce(holder IS NOT NULL AND expires_at > now(), false)
               FROM connectors.imap_idle_leases ORDER BY connection_id",
        )
        .fetch_all(&self.kit.db)
        .await
        .unwrap()
    }

    async fn health(&self) -> String {
        sqlx::query_scalar("SELECT health FROM connectors.connections")
            .fetch_one(&self.kit.db)
            .await
            .unwrap()
    }

    /// When the stream is due, from now.
    async fn due_in(&self) -> TimeDelta {
        let due: DateTime<Utc> =
            serde_json::from_value(self.kit.stream().await["due_at"].clone()).unwrap();
        due - self.kit.clock.now()
    }

    /// The subjects announced so far.
    async fn subjects(&self) -> Vec<String> {
        self.kit
            .batches(self.ws)
            .await
            .iter()
            .flat_map(|b| b["changes"].as_array().cloned().unwrap_or_default())
            .filter_map(|c| c["message"]["subject"].as_str().map(str::to_owned))
            .collect()
    }
}

/// The lease rows `tx` sees.
async fn count(tx: &mut sqlx::PgConnection) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM connectors.imap_idle_leases")
        .fetch_one(tx)
        .await
        .unwrap()
}

/// Wait (10 s at most) until `check` holds.
async fn until(what: &str, mut check: impl AsyncFnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !check().await {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

#[sqlx::test(migrations = "../../../migrations")]
async fn idle_is_off_unless_turned_on(db: PgPool) {
    let (kit, _fakes) = testkit_imap::kit(db).await;
    assert!(!kit.deps.imap.config.idle_enabled);
    assert!(IdlePool::new(kit.deps.clone(), kit.state.clone()).is_none());
    let mut deps = kit.deps.clone();
    let mut imap = (*deps.imap).clone();
    imap.config.idle_enabled = true;
    deps.imap = std::sync::Arc::new(imap);
    assert!(IdlePool::new(deps.clone(), kit.state.clone()).is_some());
    // Without IMAP itself, no IDLE either.
    let mut imap = (*deps.imap).clone();
    imap.config.enabled = false;
    deps.imap = std::sync::Arc::new(imap);
    assert!(IdlePool::new(deps, kit.state.clone()).is_none());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn new_mail_during_idle_asks_for_one_run(db: PgPool) {
    let m = mailbox(db, true, |_| {}).await;
    assert_eq!(m.leases().await, [(None, false)], "offered by the run");
    let (pool, held) = m.idling(timing()).await;
    assert_eq!(m.kit.pending("connectors.sync_stream").await, 0);
    let opened = m.imap.seen().opened;
    assert_eq!(opened.last().map(String::as_str), Some("INBOX"));
    assert!(!m.imap.seen().selected, "the Inbox is only examined");

    m.imap.deliver(
        "INBOX",
        &from_bea("order@bakery.example", "Order"),
        m.kit.clock.now(),
    );
    until("a run to be asked for", async || pool.shared().asked() >= 1).await;
    until("the run to be pending", async || {
        m.kit.pending("connectors.sync_stream").await == 1
    })
    .await;
    assert_eq!(pool.shared().asked(), 1);
    m.kit.run_jobs().await;
    assert_eq!(m.subjects().await, ["Hello", "Order"]);

    // A clean shutdown ends the session and releases the lease.
    let logouts = m.imap.seen().logouts;
    pool.shutdown(held).await;
    assert_eq!(m.leases().await, [(None, false)]);
    assert!(m.imap.seen().dones >= 1 && m.imap.seen().logouts > logouts);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_burst_of_new_mail_is_one_run(db: PgPool) {
    let m = mailbox(db, true, |_| {}).await;
    let (pool, held) = m.idling(timing()).await;
    for n in 1..=5 {
        m.imap.deliver(
            "INBOX",
            &from_bea(&format!("order-{n}@bakery.example"), "Order"),
            m.kit.clock.now(),
        );
        until("the server to report it", async || {
            m.imap.seen().reported >= n
        })
        .await;
    }
    until("the run to be pending", async || {
        m.kit.pending("connectors.sync_stream").await == 1
    })
    .await;
    assert_eq!(pool.shared().asked(), 1, "one ask in the interval");
    assert_eq!(m.kit.pending("connectors.sync_stream").await, 1);
    pool.shutdown(held).await;
}

#[sqlx::test(migrations = "../../../migrations")]
async fn idle_is_renewed_on_the_same_connection(db: PgPool) {
    let m = mailbox(db, true, |_| {}).await;
    let before = m.imap.seen().connections;
    let (pool, held) = m
        .idling(Timing {
            refresh: Duration::from_millis(200),
            ..timing()
        })
        .await;
    until("IDLE to be renewed twice", async || {
        let seen = m.imap.seen();
        seen.idles >= 3 && seen.dones >= 2 && seen.noops >= 2
    })
    .await;
    assert_eq!(m.imap.seen().connections, before + 1, "one connection");
    assert_eq!(pool.shared().asked(), 0, "nothing changed");
    pool.shutdown(held).await;
}

#[sqlx::test(migrations = "../../../migrations")]
async fn two_processes_never_both_hold_a_connection(db: PgPool) {
    let m = mailbox(db, true, |_| {}).await;
    let (a, mut held_a) = m.idling(timing()).await;
    let b = m.pool(timing());
    let mut held_b = IdlePool::held();
    b.tick(&mut held_b).await;
    assert!(held_b.connections().is_empty(), "held by the first");
    let connections = m.imap.seen().connections;

    // The first could not renew in time: its lease expires.
    sqlx::query("UPDATE connectors.imap_idle_leases SET expires_at = now() - interval '1 second'")
        .execute(&m.kit.db)
        .await
        .unwrap();
    b.tick(&mut held_b).await;
    assert_eq!(held_b.connections().len(), 1);
    assert_eq!(
        m.leases().await,
        [(Some(b.shared().holder()), true)],
        "one holder"
    );
    // The first finds it lost at its next renewal and stops.
    let logouts = m.imap.seen().logouts;
    a.tick(&mut held_a).await;
    assert!(held_a.connections().is_empty());
    until("the first session to leave", async || {
        m.imap.seen().logouts > logouts
    })
    .await;
    until("the second session to be in IDLE", async || {
        m.imap.seen().idles >= 2
    })
    .await;
    assert_eq!(m.imap.seen().connections, connections + 1);
    // Its shutdown releases nothing of the second's.
    a.shutdown(held_a).await;
    assert_eq!(m.leases().await, [(Some(b.shared().holder()), true)]);
    b.shutdown(held_b).await;
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_server_without_idle_keeps_polling(db: PgPool) {
    let m = mailbox(db, false, |_| {}).await;
    assert!(m.leases().await.is_empty(), "never offered");
    let connections = m.imap.seen().connections;
    let pool = m.pool(timing());
    let mut held = IdlePool::held();
    pool.tick(&mut held).await;
    assert!(held.connections().is_empty());
    assert_eq!(m.imap.seen().connections, connections);
    assert!(
        m.due_in().await < TimeDelta::seconds(900),
        "adaptive polling"
    );
    assert_eq!(m.kit.stream().await["cursor"]["idle"], false);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_mailbox_held_in_idle_keeps_a_safety_poll(db: PgPool) {
    let m = mailbox(db, true, |_| {}).await;
    assert!(m.due_in().await < TimeDelta::seconds(900));
    let (pool, held) = m.idling(timing()).await;
    m.kit.next_run().await;
    assert!(
        m.due_in().await >= TimeDelta::seconds(900),
        "{}",
        m.due_in().await
    );
    pool.shutdown(held).await;
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_quiet_mailbox_drops_idle(db: PgPool) {
    let m = mailbox(db, true, |_| {}).await;
    let (pool, mut held) = m.idling(timing()).await;
    sqlx::query("UPDATE connectors.streams SET last_change_at = $1")
        .bind(m.kit.clock.now() - TimeDelta::days(15))
        .execute(&m.kit.db)
        .await
        .unwrap();
    m.kit.next_run().await;
    assert!(m.leases().await.is_empty(), "withdrawn by the run");
    let logouts = m.imap.seen().logouts;
    pool.tick(&mut held).await;
    assert!(held.connections().is_empty());
    until("the session to leave", async || {
        m.imap.seen().logouts > logouts
    })
    .await;
    assert!(m.due_in().await < TimeDelta::seconds(900), "polled again");
    pool.shutdown(held).await;
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_server_repeating_exists_is_cut_off_and_backs_off(db: PgPool) {
    let m = mailbox(db, true, |_| {}).await;
    m.imap.script(|script| script.offers.idle_flood = true);
    let connections = m.imap.seen().connections;
    let pool = m.pool(timing());
    let mut held = IdlePool::held();
    pool.tick(&mut held).await;
    until("the lease to be given up", async || {
        m.leases().await == [(None, false)]
    })
    .await;
    // Three sessions, each dropped past the cap; one run asked for.
    assert_eq!(m.imap.seen().connections, connections + 3);
    assert_eq!(pool.shared().asked(), 1);
    assert_eq!(m.kit.pending("connectors.sync_stream").await, 1);
    // The cooldown keeps every pool off it for a while.
    let cooling = m
        .count(
            "SELECT count(*) FROM connectors.imap_idle_leases
              WHERE holder IS NULL AND expires_at > now() + interval '20 minutes'",
        )
        .await;
    assert_eq!(cooling, 1);
    pool.tick(&mut held).await;
    assert!(held.connections().is_empty());
    assert_ne!(m.health().await, "needs_reauth");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_reconnect_goes_through_the_guard_again(db: PgPool) {
    let m = mailbox(db, true, |_| {}).await;
    let (pool, held) = m.idling(timing()).await;
    let connections = m.imap.seen().connections;
    // The server's name now points at a cloud metadata address.
    m.fakes.resolver.name(IMAP_HOST, &["169.254.169.254"]);
    m.imap.hang_up();
    until("the lease to be given up", async || {
        m.leases().await == [(None, false)]
    })
    .await;
    assert_eq!(m.imap.seen().connections, connections, "nothing reached");
    assert_ne!(m.health().await, "needs_reauth");
    pool.shutdown(held).await;
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_changed_password_counts_with_the_runs_refusals(db: PgPool) {
    let m = mailbox(db, true, |_| {}).await;
    let (pool, held) = m.idling(timing()).await;
    // Two runs have had it refused already.
    sqlx::query(
        "UPDATE connectors.streams SET cursor = jsonb_set(cursor, '{login_refusals}', '2')",
    )
    .execute(&m.kit.db)
    .await
    .unwrap();
    m.imap.set_password("changed-at-the-provider");
    m.imap.hang_up();
    until("the connection to need the member", async || {
        m.health().await == "needs_reauth"
    })
    .await;
    let reauth = m
        .kit
        .payloads(m.ws, "connectors.connection_needs_reauth")
        .await;
    assert_eq!(reauth.len(), 1);
    assert_eq!(reauth[0]["reason"], "imap_login_refused");
    assert!(m.leases().await.is_empty(), "withdrawn");
    assert_eq!(m.kit.stream().await["paused"], true);
    pool.shutdown(held).await;
}

#[sqlx::test(migrations = "../../../migrations")]
async fn one_idle_session_per_mailbox_login(db: PgPool) {
    let m = mailbox(db, true, |_| {}).await;
    // The same mailbox connected in a second workspace.
    let other = WorkspaceId::new();
    m.kit.join(&m.ana, other, Role::Member);
    connect(&m.kit, &m.ana, other, &m.imap, &m.smtp).await;
    m.kit.run_jobs().await;
    assert_eq!(m.leases().await.len(), 2, "both offered");

    let pool = m.pool(timing());
    let mut held = IdlePool::held();
    pool.tick(&mut held).await;
    assert_eq!(held.connections().len(), 1);
    let second = m.pool(timing());
    let mut none = IdlePool::held();
    second.tick(&mut none).await;
    assert!(none.connections().is_empty());
    let live = m.leases().await.iter().filter(|(_, live)| *live).count();
    assert_eq!(live, 1);
    pool.shutdown(held).await;
}

#[sqlx::test(migrations = "../../../migrations")]
async fn leases_are_seen_only_by_their_workspace(db: PgPool) {
    let m = mailbox(db, true, |_| {}).await;
    let app = testing::app_role_pool(&m.kit.db).await;
    let mut tx = app.begin_for(m.ws).await.unwrap();
    assert_eq!(count(&mut tx).await, 1);
    tx.rollback().await.unwrap();
    // Another workspace, or none: nothing, and nothing to change.
    let mut tx = app.begin_for(WorkspaceId::new()).await.unwrap();
    assert_eq!(count(&mut tx).await, 0);
    let changed = sqlx::query(
        "UPDATE connectors.imap_idle_leases SET holder = gen_random_uuid(), expires_at = now()",
    )
    .execute(&mut *tx)
    .await
    .unwrap()
    .rows_affected();
    assert_eq!(changed, 0);
    tx.rollback().await.unwrap();
    let mut tx = app.begin().await.unwrap();
    assert_eq!(count(&mut tx).await, 0);
    tx.rollback().await.unwrap();

    // The pools' scope sees every lease, changes only holders and times,
    // and can neither insert nor delete.
    let mut tx = campbooks_kernel::db::begin_maintenance(&app).await.unwrap();
    assert_eq!(count(&mut tx).await, 1);
    let deleted = sqlx::query("DELETE FROM connectors.imap_idle_leases")
        .execute(&mut *tx)
        .await
        .unwrap()
        .rows_affected();
    assert_eq!(deleted, 0);
    let moved = sqlx::query("UPDATE connectors.imap_idle_leases SET login_key = repeat('0', 32)")
        .execute(&mut *tx)
        .await;
    assert!(moved.is_err(), "the trigger keeps the keys");
    tx.rollback().await.unwrap();
    let mut tx = campbooks_kernel::db::begin_maintenance(&app).await.unwrap();
    let inserted = sqlx::query(
        "INSERT INTO connectors.imap_idle_leases
             (connection_id, workspace_id, stream_id, login_key, host_key, provider_host)
         SELECT gen_random_uuid(), gen_random_uuid(), stream_id, login_key, host_key, false
           FROM connectors.imap_idle_leases",
    )
    .execute(&mut *tx)
    .await;
    assert!(inserted.is_err(), "no insert policy");
    tx.rollback().await.unwrap();
    // A row never moves to another workspace.
    let moved =
        sqlx::query("UPDATE connectors.imap_idle_leases SET workspace_id = gen_random_uuid()")
            .execute(&m.kit.db)
            .await;
    assert!(moved.is_err());
    // The keys are digests, never the address or the host.
    let keys: (String, String) =
        sqlx::query_as("SELECT login_key, host_key FROM connectors.imap_idle_leases")
            .fetch_one(&m.kit.db)
            .await
            .unwrap();
    assert!(!keys.0.contains("bakery") && !keys.1.contains("example"));
    assert_eq!(
        keys.1,
        lease::host_key(&m.kit.state.signer, IMAP_HOST),
        "keyed by the instance"
    );
}
