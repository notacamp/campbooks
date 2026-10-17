//! Syncing a Gmail stream against the fake Google: backfill, incremental,
//! label-only changes, deletions, expired history, throttling, revoked
//! grants, adaptive intervals, and the contract of what is produced.

use campbooks_kernel::clock::Clock as _;
use campbooks_kernel::tenancy::{Role, WorkspaceId};
use chrono::{DateTime, TimeDelta, Utc};
use sqlx::PgPool;

use crate::testkit::{FIXTURES, Kit, assert_contract, synthetic};

/// A kit with ana connected in a workspace; mail is seeded first.
async fn connected(kit: &Kit) -> (WorkspaceId, crate::testkit::Caller) {
    let ws = WorkspaceId::new();
    let ana = kit.user("ana");
    kit.join(&ana, ws, Role::Member);
    kit.connect(&ana, ws).await;
    (ws, ana)
}

fn ago(kit: &Kit, delta: TimeDelta) -> DateTime<Utc> {
    kit.clock.now() - delta
}

/// Every upsert and removal of every announced batch, validated against
/// the contract, in order.
async fn changes(kit: &Kit, ws: WorkspaceId) -> Vec<serde_json::Value> {
    for event in kit.payloads(ws, "connectors.messages_changed").await {
        assert_contract("connectors.messages_changed.v1.schema.json", &event);
    }
    let mut all = Vec::new();
    for batch in kit.batches(ws).await {
        assert_contract("message-change-batch.v1.schema.json", &batch);
        all.extend(batch["changes"].as_array().unwrap().iter().cloned());
    }
    all
}

fn upsert_of<'a>(changes: &'a [serde_json::Value], id: &str) -> &'a serde_json::Value {
    &changes
        .iter()
        .rev()
        .find(|c| c["op"] == "upsert" && c["message"]["provider_message_id"] == id)
        .unwrap_or_else(|| panic!("no upsert of {id}"))["message"]
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_recorded_fixtures_are_normalised(db: PgPool) {
    let kit = Kit::new(db).await;
    let ids: Vec<String> = FIXTURES
        .iter()
        .enumerate()
        .map(|(i, (_, raw))| {
            let labels: &[&str] = match i {
                0 => &["INBOX", "UNREAD", "STARRED", "Label_7"],
                1 => &["INBOX"],
                2 => &["INBOX", "CATEGORY_PROMOTIONS", "UNREAD"],
                _ => &["CATEGORY_UPDATES", "Label_9"],
            };
            kit.google.seed(
                raw,
                labels,
                ago(&kit, TimeDelta::hours(i64::try_from(i).unwrap() + 1)),
            )
        })
        .collect();
    let (ws, _) = connected(&kit).await;
    kit.run_jobs().await;
    let all = changes(&kit, ws).await;
    assert_eq!(all.len(), 4);

    let invoice = upsert_of(&all, &ids[0]);
    assert_eq!(invoice["from"]["address"], "olivia@northwind.example.com");
    assert_eq!(invoice["subject"], "Invoice INV-2041 for September");
    assert_eq!(invoice["rfc_message_id"], "inv-2041@northwind.example.com");
    assert_eq!(invoice["thread_key"], format!("thread-{}", ids[0]));
    assert_eq!(invoice["direction"], "inbound");
    assert_eq!(invoice["read"], false);
    assert_eq!(invoice["starred"], true);
    assert_eq!(
        invoice["labels"],
        serde_json::json!([{"key": "Label_7", "name": "Clients"}])
    );
    assert_eq!(invoice["attachments"][0]["filename"], "INV-2041.pdf");
    assert_eq!(invoice["attachments"][0]["inline"], false);
    let text: campbooks_kernel::blobs::BlobRef =
        serde_json::from_value(invoice["text"].clone()).unwrap();
    assert!(
        String::from_utf8(kit.blob(ws, &text).await)
            .unwrap()
            .contains("INV-2041")
    );
    assert!(invoice["html"].is_object() && invoice["raw"].is_object());

    let order = upsert_of(&all, &ids[1]);
    assert_eq!(order["subject"], "Encomenda para sábado");
    assert_eq!(order["in_reply_to"], "pedido-76@example.com");
    assert!(order["html"].is_null());
    assert_eq!(order["read"], true);

    let newsletter = upsert_of(&all, &ids[2]);
    assert_eq!(newsletter["subject"], "Novedades de otoño");
    assert_eq!(newsletter["categories"], serde_json::json!(["promotions"]));
    assert_eq!(newsletter["headers"]["precedence"], "bulk");
    assert_eq!(
        newsletter["headers"]["list_unsubscribe"],
        "<https://cafesol.example.com/baja>"
    );

    let statement = upsert_of(&all, &ids[3]);
    assert_eq!(statement["mailbox"]["in_inbox"], false);
    assert_eq!(statement["categories"], serde_json::json!(["updates"]));
    assert_eq!(statement["labels"][0]["name"], "Receipts");
    assert_eq!(statement["headers"]["auto_submitted"], "auto-generated");
    assert_eq!(statement["attachments"][0]["inline"], true);

    // Every blob a message references is held by the index.
    let held: i64 = sqlx::query_scalar!(
        r#"SELECT count(DISTINCT hash) AS "n!" FROM kernel.blob_refs
            WHERE owner_type = 'connectors.gmail_message'"#
    )
    .fetch_one(&kit.db)
    .await
    .unwrap();
    assert!(held >= 4 * 3, "{held}");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn backfill_in_pages_then_incremental(db: PgPool) {
    let kit = Kit::new(db).await;
    for n in 0..120 {
        kit.google.seed(
            &synthetic(n),
            &["INBOX"],
            ago(
                &kit,
                TimeDelta::hours(1) + TimeDelta::minutes(i64::try_from(n).unwrap()),
            ),
        );
    }
    // Outside the 90-day window.
    kit.google
        .seed(&synthetic(999), &["INBOX"], ago(&kit, TimeDelta::days(100)));
    let (ws, _) = connected(&kit).await;
    kit.run_jobs().await;

    let batches = kit.batches(ws).await;
    let sizes: Vec<usize> = batches
        .iter()
        .map(|b| b["changes"].as_array().unwrap().len())
        .collect();
    assert_eq!(sizes, [50, 50, 20]);
    // Oldest first within a batch.
    let first = &batches[0]["changes"];
    assert!(first[0]["message"]["sent_at"].as_str() < first[49]["message"]["sent_at"].as_str());
    assert_eq!(changes(&kit, ws).await.len(), 120);
    let stream = kit.stream().await;
    assert_eq!(stream["phase"], "incremental");

    // New mail is read from history.
    let a = kit
        .google
        .deliver(&synthetic(200), &["INBOX", "UNREAD"], kit.clock.now());
    let b = kit
        .google
        .deliver(&synthetic(201), &["INBOX", "UNREAD"], kit.clock.now());
    assert_eq!(kit.next_run().await, 1);
    let batches = kit.batches(ws).await;
    let last = &batches.last().unwrap()["changes"];
    let ids: Vec<&str> = last
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["message"]["provider_message_id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, [a.as_str(), b.as_str()]);
    let events = kit.payloads(ws, "connectors.messages_changed").await;
    assert_eq!(
        events.last().unwrap()["inbox_id"],
        kit.stream().await["inbox_id"]
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn label_only_changes_are_not_downloaded_again(db: PgPool) {
    let kit = Kit::new(db).await;
    let id = kit.google.seed(
        FIXTURES[0].1,
        &["INBOX", "UNREAD"],
        ago(&kit, TimeDelta::hours(2)),
    );
    let (ws, _) = connected(&kit).await;
    kit.run_jobs().await;
    assert_eq!(kit.google.raw_fetches(&id), 1);

    // Starred and labelled, read.
    kit.google
        .relabel(&id, &["STARRED", "Label_7"], &["UNREAD"]);
    kit.next_run().await;
    let all = changes(&kit, ws).await;
    let now = upsert_of(&all, &id);
    assert_eq!(
        (now["read"].clone(), now["starred"].clone()),
        (true.into(), true.into())
    );
    assert_eq!(now["labels"][0]["name"], "Clients");
    assert_eq!(now["subject"], "Invoice INV-2041 for September");

    // Archived in Gmail.
    kit.google.relabel(&id, &[], &["INBOX"]);
    kit.next_run().await;
    let all = changes(&kit, ws).await;
    assert_eq!(upsert_of(&all, &id)["mailbox"]["in_inbox"], false);
    assert_eq!(kit.google.raw_fetches(&id), 1, "never downloaded again");
    assert_eq!(all.len(), 3);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn deletions_become_removals(db: PgPool) {
    let kit = Kit::new(db).await;
    let id = kit
        .google
        .seed(&synthetic(1), &["INBOX"], ago(&kit, TimeDelta::hours(2)));
    let (ws, _) = connected(&kit).await;
    kit.run_jobs().await;
    kit.google.delete(&id);
    kit.next_run().await;
    let all = changes(&kit, ws).await;
    assert_eq!(
        all.last().unwrap(),
        &serde_json::json!({"op": "remove", "provider_message_id": id})
    );
    let indexed: i64 = sqlx::query_scalar!(r#"SELECT count(*) AS "n!" FROM connectors.messages"#)
        .fetch_one(&kit.db)
        .await
        .unwrap();
    assert_eq!(indexed, 0);
    // Its blobs are released (the sweeper may delete them after the grace).
    let held: i64 = sqlx::query_scalar!(
        r#"SELECT count(*) AS "n!" FROM kernel.blob_refs WHERE owner_type = 'connectors.gmail_message'"#
    )
    .fetch_one(&kit.db)
    .await
    .unwrap();
    assert_eq!(held, 0);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn expired_history_resyncs_the_window(db: PgPool) {
    let kit = Kit::new(db).await;
    let keep = kit
        .google
        .seed(&synthetic(1), &["INBOX"], ago(&kit, TimeDelta::hours(3)));
    let gone = kit
        .google
        .seed(&synthetic(2), &["INBOX"], ago(&kit, TimeDelta::hours(2)));
    let (ws, _) = connected(&kit).await;
    kit.run_jobs().await;
    let fresh = kit
        .google
        .seed(&synthetic(3), &["INBOX"], ago(&kit, TimeDelta::hours(1)));
    kit.google.vanish(&gone);
    kit.google.relabel(&keep, &["STARRED"], &[]);
    kit.google.expire_history();

    kit.next_run().await;
    let all = changes(&kit, ws).await;
    let after_backfill = &all[2..];
    assert!(
        after_backfill.contains(&serde_json::json!({"op": "remove", "provider_message_id": gone}))
    );
    assert_eq!(
        upsert_of(after_backfill, &fresh)["provider_message_id"],
        fresh
    );
    assert_eq!(upsert_of(after_backfill, &keep)["starred"], true);
    assert_eq!(kit.google.raw_fetches(&keep), 1);
    assert_eq!(kit.stream().await["phase"], "incremental");

    // And history works again from the new id.
    let next = kit
        .google
        .deliver(&synthetic(4), &["INBOX"], kit.clock.now());
    kit.next_run().await;
    assert_eq!(
        upsert_of(&changes(&kit, ws).await, &next)["provider_message_id"],
        next
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn throttling_pauses_only_until_retry_after(db: PgPool) {
    let kit = Kit::new(db).await;
    let (ws, ana) = connected(&kit).await;
    kit.run_jobs().await;
    let id = kit
        .google
        .deliver(&synthetic(1), &["INBOX"], kit.clock.now());
    kit.google.rate_limit_next(120);
    kit.next_run().await;

    let connection = &kit
        .get(&ana, &format!("/api/v1/w/{ws}/connections"))
        .await
        .json["connections"][0];
    assert_eq!(connection["health"], "throttled");
    assert_eq!(connection["health_reason"], "rate_limited");
    let due: DateTime<Utc> = serde_json::from_value(kit.stream().await["due_at"].clone()).unwrap();
    assert_eq!(due, kit.clock.now() + TimeDelta::seconds(120));
    // Not before.
    assert_eq!(kit.tick(TimeDelta::seconds(60)).await, 0);
    kit.next_run().await;
    upsert_of(&changes(&kit, ws).await, &id);
    let connection = &kit
        .get(&ana, &format!("/api/v1/w/{ws}/connections"))
        .await
        .json["connections"][0];
    assert_eq!(connection["health"], "ok");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_revoked_grant_needs_reauth_once(db: PgPool) {
    let kit = Kit::new(db).await;
    let (ws, ana) = connected(&kit).await;
    kit.run_jobs().await;
    kit.google.revoke_grant();
    // The access token expires; the refresh is refused.
    kit.tick(TimeDelta::hours(2)).await;
    let connection = &kit
        .get(&ana, &format!("/api/v1/w/{ws}/connections"))
        .await
        .json["connections"][0];
    assert_eq!(connection["health"], "needs_reauth");
    assert_eq!(connection["health_reason"], "grant_revoked");
    assert_eq!(connection["streams"][0]["paused"], true);
    assert!(connection["streams"][0]["next_sync_at"].is_null());
    let events = kit.payloads(ws, "connectors.connection_needs_reauth").await;
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["reason"], "grant_revoked");
    // Nothing runs any more; sync now is refused.
    assert_eq!(kit.tick(TimeDelta::hours(5)).await, 0);
    let id = connection["id"].as_str().unwrap();
    let sync = kit
        .post(
            &ana,
            &format!("/api/v1/w/{ws}/connections/{id}/sync"),
            serde_json::json!({}),
        )
        .await;
    assert_eq!(sync.json["code"], "needs_reauth");

    // Connecting again resumes it.
    kit.connect(&ana, ws).await;
    let connection = &kit
        .get(&ana, &format!("/api/v1/w/{ws}/connections"))
        .await
        .json["connections"][0];
    assert_eq!(connection["health"], "ok");
    assert_eq!(connection["streams"][0]["paused"], false);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_refused_access_token_is_refreshed(db: PgPool) {
    let kit = Kit::new(db).await;
    let (ws, ana) = connected(&kit).await;
    kit.run_jobs().await;
    let id = kit
        .google
        .deliver(&synthetic(1), &["INBOX"], kit.clock.now());
    kit.google.unauthorized_next();
    kit.next_run().await;
    kit.next_run().await;
    upsert_of(&changes(&kit, ws).await, &id);
    let connection = &kit
        .get(&ana, &format!("/api/v1/w/{ws}/connections"))
        .await
        .json["connections"][0];
    assert_eq!(connection["health"], "ok");
    assert!(kit.google.calls().iter().any(|c| c == "POST /token"));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_idle_stream_backs_off_to_the_maximum(db: PgPool) {
    let mut config = crate::config::ConnectorsConfig::default();
    config.poll.max_seconds = 600;
    let kit = Kit::with_config(db, config).await;
    connected(&kit).await;
    kit.run_jobs().await;
    let mut intervals = Vec::new();
    for _ in 0..5 {
        kit.next_run().await;
        intervals.push(kit.stream().await["interval_seconds"].as_i64().unwrap());
    }
    assert_eq!(intervals, [240, 480, 600, 600, 600]);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_large_backfill_does_not_hold_up_another_workspace(db: PgPool) {
    let kit = Kit::new(db).await;
    for n in 0..120 {
        kit.google.seed(
            &synthetic(n),
            &["INBOX"],
            ago(
                &kit,
                TimeDelta::hours(1) + TimeDelta::minutes(i64::try_from(n).unwrap()),
            ),
        );
    }
    let (a, _) = connected(&kit).await;
    let b = WorkspaceId::new();
    let bea = kit.user("bea");
    kit.join(&bea, b, Role::Member);
    kit.connect(&bea, b).await;
    kit.run_jobs().await;

    // Each run reads one page, so the queue interleaves the workspaces:
    // b's first batch lands before a's backfill is over.
    let order: Vec<String> = sqlx::query_scalar!(
        r#"SELECT workspace_id AS "w!" FROM kernel.events
            WHERE name = 'connectors.messages_changed' ORDER BY id"#
    )
    .fetch_all(&kit.db)
    .await
    .unwrap();
    assert_eq!(order.len(), 6);
    let first_b = order.iter().position(|w| *w == b.to_string()).unwrap();
    let last_a = order.iter().rposition(|w| *w == a.to_string()).unwrap();
    assert!(first_b < last_a, "{order:?}");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_run_stops_at_its_byte_budget_and_the_next_one_resumes(db: PgPool) {
    let mut config = crate::config::ConnectorsConfig::default();
    // Every download goes over: a run fetches only what it already started
    // (the concurrency, 5), then stops.
    config.gmail.run_budget_bytes = 1;
    let kit = Kit::with_config(db, config).await;
    let mut seeded = Vec::new();
    for n in 0..12 {
        seeded.push(kit.google.seed(
            &synthetic(n),
            &["INBOX"],
            ago(
                &kit,
                TimeDelta::hours(1) + TimeDelta::minutes(i64::try_from(n).unwrap()),
            ),
        ));
    }
    let (ws, _) = connected(&kit).await;
    kit.run_jobs().await;
    let sizes: Vec<usize> = kit
        .batches(ws)
        .await
        .iter()
        .map(|b| b["changes"].as_array().unwrap().len())
        .collect();
    assert_eq!(sizes, [5, 5, 2]);
    let mut ids: Vec<String> = changes(&kit, ws)
        .await
        .iter()
        .map(|c| {
            c["message"]["provider_message_id"]
                .as_str()
                .unwrap()
                .to_owned()
        })
        .collect();
    ids.sort();
    seeded.sort();
    assert_eq!(ids, seeded, "every message once");
    assert!(seeded.iter().all(|id| kit.google.raw_fetches(id) == 1));
    assert_eq!(kit.stream().await["phase"], "incremental");

    // History too: seven new messages over two runs, none lost or repeated.
    let mut delivered: Vec<String> = (100..107)
        .map(|n| {
            kit.google
                .deliver(&synthetic(n), &["INBOX"], kit.clock.now())
        })
        .collect();
    kit.next_run().await;
    let mut new_ids: Vec<String> = changes(&kit, ws).await[12..]
        .iter()
        .map(|c| {
            c["message"]["provider_message_id"]
                .as_str()
                .unwrap()
                .to_owned()
        })
        .collect();
    new_ids.sort();
    delivered.sort();
    assert_eq!(new_ids, delivered);
    assert!(delivered.iter().all(|id| kit.google.raw_fetches(id) == 1));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn oversized_messages_are_skipped_and_never_fetched_again(db: PgPool) {
    let mut config = crate::config::ConnectorsConfig::default();
    config.gmail.max_message_bytes = 2_000;
    let kit = Kit::with_config(db, config).await;
    let mut big = synthetic(1);
    big.extend(std::iter::repeat_n(b'x', 10_000));
    let big = kit
        .google
        .seed(&big, &["INBOX"], ago(&kit, TimeDelta::hours(2)));
    let small = kit
        .google
        .seed(&synthetic(2), &["INBOX"], ago(&kit, TimeDelta::hours(1)));
    let (ws, _) = connected(&kit).await;
    kit.run_jobs().await;
    let all = changes(&kit, ws).await;
    assert_eq!(all.len(), 1);
    upsert_of(&all, &small);
    let recorded: Vec<String> =
        sqlx::query_scalar!("SELECT provider_message_id FROM connectors.oversized")
            .fetch_all(&kit.db)
            .await
            .unwrap();
    assert_eq!(recorded, std::slice::from_ref(&big));
    // A resync of the window does not download it again.
    kit.google.expire_history();
    kit.next_run().await;
    assert_eq!(kit.google.raw_fetches(&big), 1);
}

/// The dispatcher works in the maintenance scope (campbooks-cloud#196): it
/// leases the due streams of every workspace and enqueues each run in its
/// own workspace, as the runtime role.
#[sqlx::test(migrations = "../../../migrations")]
async fn the_dispatcher_runs_every_workspace_s_due_streams(db: PgPool) {
    let kit = Kit::new(db.clone()).await;
    let (ana_ws, _) = connected(&kit).await;
    let bea_ws = WorkspaceId::new();
    let bea = kit.user("bea");
    kit.join(&bea, bea_ws, Role::Member);
    kit.adopt(&bea, bea_ws, "bea-account", None)
        .await
        .unwrap_or_else(|e| panic!("adopting: {e:?}"));
    kit.run_jobs().await;
    let runs = |ws: WorkspaceId| {
        let db = db.clone();
        async move {
            sqlx::query_scalar::<_, i64>(
                "SELECT count(*) FROM kernel.jobs
                  WHERE kind = 'connectors.sync_stream' AND workspace_id = $1
                    AND state = 'succeeded'",
            )
            .bind(ws.to_string())
            .fetch_one(&db)
            .await
            .unwrap()
        }
    };
    let before = (runs(ana_ws).await, runs(bea_ws).await);
    assert_eq!(kit.tick(TimeDelta::days(1)).await, 2);
    assert_eq!(
        (runs(ana_ws).await, runs(bea_ws).await),
        (before.0 + 1, before.1 + 1),
        "one more run in each workspace"
    );
}

/// Another workspace's row holding a stream's job key (raw SQL from its own
/// transaction, which the policies admit: its own workspace, no schedule
/// key), never due, no longer stops that stream's sync: job keys are per
/// workspace (campbooks-cloud#209).
#[sqlx::test(migrations = "../../../migrations")]
async fn another_workspace_cannot_hold_a_stream_s_runs(db: PgPool) {
    use campbooks_kernel::db::BeginFor as _;

    let kit = Kit::new(db.clone()).await;
    let (ana_ws, _) = connected(&kit).await;
    kit.run_jobs().await;
    let stream = kit.stream().await["id"].as_str().unwrap().to_owned();
    let key = format!("connectors.sync_stream:{stream}");
    let intruder = WorkspaceId::new();
    let mut tx = kit.state.db.begin_for(intruder).await.unwrap();
    sqlx::query(
        "INSERT INTO kernel.jobs (queue, kind, payload, workspace_id, unique_key, ordering_key,
                                  run_at, max_attempts)
         VALUES ('sync', 'connectors.sync_stream', jsonb_build_object('stream', $2::text), $1,
                 $3, $3, 'infinity', 1)",
    )
    .bind(intruder.to_string())
    .bind(&stream)
    .bind(&key)
    .execute(&mut *tx)
    .await
    .unwrap();
    tx.commit().await.unwrap();

    let runs = || {
        let db = db.clone();
        async move {
            sqlx::query_scalar::<_, i64>(
                "SELECT count(*) FROM kernel.jobs
                  WHERE kind = 'connectors.sync_stream' AND workspace_id = $1
                    AND state = 'succeeded'",
            )
            .bind(ana_ws.to_string())
            .fetch_one(&db)
            .await
            .unwrap()
        }
    };
    let before = runs().await;
    assert_eq!(kit.tick(TimeDelta::days(1)).await, 1);
    assert_eq!(
        runs().await,
        before + 1,
        "the stream's run was not held back"
    );
}
