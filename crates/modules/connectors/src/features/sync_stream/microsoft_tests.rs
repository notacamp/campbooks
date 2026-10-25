//! A Microsoft stream (campbooks-cloud#316): each folder's Graph delta read
//! into the contract, against the fake Graph; and the grant's health, as
//! #315 kept it.

use campbooks_kernel::clock::Clock as _;
use campbooks_kernel::tenancy::{Role, WorkspaceId};
use chrono::{DateTime, TimeDelta, Utc};
use serde_json::Value;
use sqlx::PgPool;

use crate::testkit::{Caller, Kit, assert_contract, synthetic};

/// A kit with Microsoft mail on and ana's Outlook connected, its first sync
/// done.
async fn connected(db: PgPool) -> (Kit, WorkspaceId, Caller) {
    connected_with(Kit::with_microsoft(db).await).await
}

async fn connected_with(kit: Kit) -> (Kit, WorkspaceId, Caller) {
    let ws = WorkspaceId::new();
    let ana = kit.user("ana");
    kit.join(&ana, ws, Role::Member);
    kit.connect_microsoft(&ana, ws).await;
    kit.run_jobs().await;
    (kit, ws, ana)
}

async fn connection(kit: &Kit, ws: WorkspaceId, ana: &Caller) -> Value {
    kit.get(ana, &format!("/api/v1/w/{ws}/connections"))
        .await
        .json["connections"][0]
        .clone()
}

async fn due(kit: &Kit) -> DateTime<Utc> {
    serde_json::from_value(kit.stream().await["due_at"].clone()).unwrap()
}

/// Every change announced so far, in order.
async fn changes(kit: &Kit, ws: WorkspaceId) -> Vec<Value> {
    kit.batches(ws)
        .await
        .into_iter()
        .flat_map(|batch| batch["changes"].as_array().cloned().unwrap_or_default())
        .collect()
}

/// The newest upsert of `id` among `changes`.
fn upsert<'a>(changes: &'a [Value], id: &str) -> Option<&'a Value> {
    changes
        .iter()
        .rev()
        .find(|c| c["op"] == "upsert" && c["message"]["provider_message_id"] == id)
        .map(|c| &c["message"])
}

fn upserts(changes: &[Value], id: &str) -> usize {
    changes
        .iter()
        .filter(|c| c["op"] == "upsert" && c["message"]["provider_message_id"] == id)
        .count()
}

fn removed(changes: &[Value], id: &str) -> bool {
    changes
        .iter()
        .any(|c| c["op"] == "remove" && c["provider_message_id"] == id)
}

/// Only reads were made at Graph: no write, move or delete.
fn read_only(kit: &Kit) {
    for call in kit.microsoft.calls() {
        let graph = call.contains(" /v1.0/");
        let subscription = call.contains(" /v1.0/subscriptions");
        assert!(
            !graph || subscription || call.starts_with("GET "),
            "a write to Microsoft: {call}"
        );
    }
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_backfill_reads_every_folder_but_drafts_then_only_new_mail(db: PgPool) {
    let kit = Kit::with_microsoft(db).await;
    let hour_ago = kit.clock.now() - TimeDelta::hours(1);
    let fake = &kit.microsoft;
    let clients = fake.add_folder("Clients", None);
    let updates_parent = fake.add_folder("Campbooks", Some("fld-inbox"));
    let updates = fake.add_folder("Updates", Some(&updates_parent));
    let inbox = fake.deliver("fld-inbox", &synthetic(1), hour_ago);
    fake.change(&inbox, |m| m.categories = vec!["Red".to_owned()]);
    let sent = fake.deliver("fld-sent", &synthetic(2), hour_ago);
    let filed = fake.deliver(&clients, &synthetic(3), hour_ago);
    let stream_mail = fake.deliver(&updates, &synthetic(4), hour_ago);
    let draft = fake.deliver("fld-drafts", &synthetic(5), hour_ago);
    let search = fake.add_search_folder("Weekly digests", Some("fld-inbox"));
    let old = fake.deliver(
        "fld-inbox",
        &synthetic(6),
        kit.clock.now() - TimeDelta::days(200),
    );
    let (kit, ws, ana) = connected_with(kit).await;

    let all = changes(&kit, ws).await;
    for batch in kit.batches(ws).await {
        assert_contract("message-change-batch.v1.schema.json", &batch);
        assert_eq!(batch["backfill"], true);
    }
    for id in [&inbox, &sent, &filed, &stream_mail] {
        assert_eq!(upserts(&all, id), 1, "{id} once");
        assert_eq!(kit.microsoft.raw_fetches(id), 1);
    }
    // Drafts are never read, and mail older than the window stays out.
    assert_eq!(upserts(&all, &draft), 0);
    assert_eq!(kit.microsoft.raw_fetches(&draft), 0);
    assert_eq!(upserts(&all, &old), 0);
    assert!(
        !kit.microsoft
            .calls()
            .iter()
            .any(|c| c.contains("/mailFolders/fld-drafts/messages")),
        "Drafts' delta is never read"
    );
    assert!(
        !kit.microsoft
            .calls()
            .iter()
            .any(|c| c.contains(&format!("/mailFolders/{search}/messages"))),
        "a search folder (a view of others) is never read"
    );

    let inbox_msg = upsert(&all, &inbox).unwrap();
    assert_eq!(inbox_msg["mailbox"]["in_inbox"], true);
    assert_eq!(inbox_msg["read"], false);
    assert_eq!(inbox_msg["direction"], "inbound");
    assert_eq!(inbox_msg["subject"], "Message 1");
    assert_eq!(inbox_msg["thread_key"], format!("conv-{inbox}"));
    assert_eq!(inbox_msg["provider_stream"], "primary");
    assert_eq!(inbox_msg["labels"][0]["name"], "Red");
    assert_eq!(upsert(&all, &sent).unwrap()["direction"], "outbound");
    let filed_msg = upsert(&all, &filed).unwrap();
    assert_eq!(filed_msg["mailbox"]["in_inbox"], false);
    assert_eq!(filed_msg["labels"][0]["name"], "Clients");
    // `Inbox/Campbooks/Updates` is the Updates stream's folder.
    let stream_msg = upsert(&all, &stream_mail).unwrap();
    assert_eq!(stream_msg["mailbox"]["in_inbox"], true);
    assert_eq!(stream_msg["provider_stream"], "updates");

    let stream = kit.stream().await;
    assert_eq!(stream["phase"], "incremental");
    assert_eq!(connection(&kit, ws, &ana).await["health"], "ok");

    // A new message: the next run announces it alone.
    let before = all.len();
    let new = kit
        .microsoft
        .deliver("fld-inbox", &synthetic(7), kit.clock.now());
    kit.next_run().await;
    let all = changes(&kit, ws).await;
    assert_eq!(all.len(), before + 1);
    assert_eq!(upserts(&all, &new), 1);
    let last = kit.batches(ws).await.pop().unwrap();
    assert!(last.get("backfill").is_none());
    // Reading marks nothing read at Microsoft.
    assert!(!kit.microsoft.message(&new).unwrap().read);
    read_only(&kit);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_folder_deleted_in_outlook_takes_its_mail_out(db: PgPool) {
    let kit = Kit::with_microsoft(db).await;
    let at = kit.clock.now() - TimeDelta::hours(1);
    let clients = kit.microsoft.add_folder("Clients", None);
    let filed = kit.microsoft.deliver(&clients, &synthetic(1), at);
    let inbox = kit.microsoft.deliver("fld-inbox", &synthetic(2), at);
    let (kit, ws, _) = connected_with(kit).await;
    kit.microsoft.remove_folder(&clients);
    kit.next_run().await;
    kit.run_jobs().await;
    let all = changes(&kit, ws).await;
    assert!(removed(&all, &filed));
    assert!(!removed(&all, &inbox));
    let stream = kit.stream().await;
    assert_eq!(stream["phase"], "incremental");
    let folders = stream["cursor"]["folders"].as_array().unwrap().clone();
    assert!(folders.iter().all(|f| f["id"] != clients.as_str()));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn delta_pages_follow_next_links_then_the_delta_link(db: PgPool) {
    let kit = Kit::with_microsoft(db).await;
    kit.microsoft.page_size(2);
    let at = kit.clock.now() - TimeDelta::hours(1);
    let ids: Vec<String> = (1..=5)
        .map(|n| kit.microsoft.deliver("fld-inbox", &synthetic(n), at))
        .collect();
    let (kit, ws, _) = connected_with(kit).await;
    let all = changes(&kit, ws).await;
    for id in &ids {
        assert_eq!(upserts(&all, id), 1);
    }
    let inbox_pages = kit
        .microsoft
        .calls()
        .iter()
        .filter(|c| c.contains("/mailFolders/fld-inbox/messages/delta"))
        .count();
    assert_eq!(
        inbox_pages, 3,
        "two pages with a nextLink, then the deltaLink"
    );
    let cursor = kit.stream().await["cursor"].clone();
    let inbox = cursor["folders"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["id"] == "fld-inbox")
        .unwrap()
        .clone();
    assert!(inbox["delta"].as_str().unwrap().starts_with('v'));
    assert!(inbox.get("skip").is_none());
    // Only the token is kept, never Graph's link.
    assert!(!cursor.to_string().contains("graph.microsoft.com"));
    assert_eq!(kit.stream().await["phase"], "incremental");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn read_moved_and_deleted_in_outlook(db: PgPool) {
    let kit = Kit::with_microsoft(db).await;
    let at = kit.clock.now() - TimeDelta::hours(1);
    let clients = kit.microsoft.add_folder("Clients", None);
    let read = kit.microsoft.deliver("fld-inbox", &synthetic(1), at);
    let moved = kit.microsoft.deliver("fld-inbox", &synthetic(2), at);
    let deleted = kit.microsoft.deliver("fld-inbox", &synthetic(3), at);
    let (kit, ws, _) = connected_with(kit).await;

    kit.microsoft.change(&read, |m| {
        m.read = true;
        m.flagged = true;
    });
    kit.next_run().await;
    let all = changes(&kit, ws).await;
    let message = upsert(&all, &read).unwrap();
    assert_eq!(message["read"], true);
    assert_eq!(message["starred"], true);
    assert_eq!(message["subject"], "Message 1", "from the stored document");

    kit.microsoft.move_to(&moved, &clients);
    kit.next_run().await;
    let all = changes(&kit, ws).await;
    let message = upsert(&all, &moved).unwrap();
    assert_eq!(message["mailbox"]["in_inbox"], false);
    assert_eq!(message["labels"][0]["name"], "Clients");
    assert!(!removed(&all, &moved), "a move keeps the message");

    kit.microsoft.delete(&deleted);
    kit.next_run().await;
    let all = changes(&kit, ws).await;
    assert!(removed(&all, &deleted));
    // Each was downloaded once, whatever happened to it.
    for id in [&read, &moved, &deleted] {
        assert_eq!(kit.microsoft.raw_fetches(id), 1, "{id}");
    }
    read_only(&kit);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_expired_delta_reads_the_window_again(db: PgPool) {
    let kit = Kit::with_microsoft(db).await;
    let at = kit.clock.now() - TimeDelta::hours(1);
    let kept = kit.microsoft.deliver("fld-inbox", &synthetic(1), at);
    let gone = kit.microsoft.deliver("fld-inbox", &synthetic(2), at);
    let (kit, ws, _) = connected_with(kit).await;
    // Deleted while Graph forgot the folder's delta: no `@removed` is ever
    // read for it.
    kit.microsoft.delete(&gone);
    kit.microsoft.expire_delta("fld-inbox");
    kit.next_run().await;
    // The resync runs to its end at once.
    kit.run_jobs().await;
    let stream = kit.stream().await;
    assert_eq!(stream["phase"], "incremental");
    assert_eq!(stream["cursor"]["generation"], 1);
    let all = changes(&kit, ws).await;
    assert!(
        removed(&all, &gone),
        "what the pass did not list is removed"
    );
    assert!(!removed(&all, &kept));
    assert_eq!(
        kit.microsoft.raw_fetches(&kept),
        1,
        "kept, not downloaded again"
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn graph_throttling_waits_for_retry_after_at_most_an_hour(db: PgPool) {
    let (kit, ws, ana) = connected(db).await;
    kit.microsoft.throttle_next(120);
    kit.next_run().await;
    let throttled = connection(&kit, ws, &ana).await;
    assert_eq!(throttled["health"], "throttled");
    assert_eq!(throttled["health_reason"], "rate_limited");
    assert_eq!(due(&kit).await, kit.clock.now() + TimeDelta::seconds(120));
    assert_eq!(kit.tick(TimeDelta::seconds(60)).await, 0, "not before");

    kit.microsoft.throttle_next(86_400);
    kit.next_run().await;
    assert_eq!(due(&kit).await, kit.clock.now() + TimeDelta::hours(1));

    kit.next_run().await;
    assert_eq!(connection(&kit, ws, &ana).await["health"], "ok");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_message_over_the_limit_is_recorded_and_never_downloaded_again(db: PgPool) {
    let mut config = Kit::microsoft_config();
    config.microsoft.max_message_bytes = Some("2048".into());
    let kit = Kit::with_config(db, config).await;
    let mut big = synthetic(1);
    big.extend(std::iter::repeat_n(b'x', 4096));
    let at = kit.clock.now() - TimeDelta::hours(1);
    let large = kit.microsoft.deliver("fld-inbox", &big, at);
    let small = kit.microsoft.deliver("fld-inbox", &synthetic(2), at);
    let (kit, ws, _) = connected_with(kit).await;
    let all = changes(&kit, ws).await;
    assert_eq!(upserts(&all, &large), 0);
    assert_eq!(upserts(&all, &small), 1);
    let oversized: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM connectors.oversized WHERE provider_message_id = $1",
    )
    .bind(&large)
    .fetch_one(&kit.db)
    .await
    .unwrap();
    assert_eq!(oversized, 1);
    // Read in Outlook: listed again, not downloaded again.
    kit.microsoft.change(&large, |m| m.read = true);
    kit.next_run().await;
    assert_eq!(kit.microsoft.raw_fetches(&large), 1);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_healthy_grant_reads_without_refreshing(db: PgPool) {
    let (kit, ws, ana) = connected(db).await;
    // The stored access token is still valid: no refresh.
    assert_eq!(kit.microsoft.token_calls(), 0);
    assert_eq!(connection(&kit, ws, &ana).await["health"], "ok");
    // Once it has expired, the run refreshes it (and keeps the rotated
    // refresh token).
    kit.clock.advance(TimeDelta::hours(2));
    kit.next_run().await;
    assert_eq!(kit.microsoft.token_calls(), 1);
    assert_eq!(connection(&kit, ws, &ana).await["health"], "ok");
    kit.clock.advance(TimeDelta::hours(2));
    kit.next_run().await;
    assert_eq!(kit.microsoft.token_calls(), 2, "the rotated token works");
    assert_eq!(connection(&kit, ws, &ana).await["health"], "ok");
    // Graph refusing the access token: dropped, refreshed by the next run.
    kit.microsoft.expire_access_tokens();
    kit.next_run().await;
    kit.next_run().await;
    assert_eq!(kit.microsoft.token_calls(), 3);
    assert_eq!(connection(&kit, ws, &ana).await["health"], "ok");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_grant_microsoft_no_longer_honours_needs_reauth(db: PgPool) {
    let (kit, ws, ana) = connected(db).await;
    // The access token expires: the next run refreshes it.
    kit.clock.advance(TimeDelta::hours(2));
    kit.microsoft.refuse_refresh(
        400,
        serde_json::json!({
            "error": "invalid_grant",
            "error_codes": [70000],
            "error_description": "AADSTS70000: The provided grant has expired due to it being revoked."
        }),
    );
    kit.next_run().await;
    let connection = connection(&kit, ws, &ana).await;
    assert_eq!(connection["health"], "needs_reauth");
    assert_eq!(connection["health_reason"], "grant_revoked");
    assert_eq!(connection["streams"][0]["paused"], true);
    let events = kit.payloads(ws, "connectors.connection_needs_reauth").await;
    assert_eq!(events.len(), 1);
    // Nothing runs any more.
    assert_eq!(kit.tick(TimeDelta::hours(5)).await, 0);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn consent_withdrawn_at_microsoft_needs_reauth(db: PgPool) {
    let (kit, ws, ana) = connected(db).await;
    // The access token expires: the next run refreshes it.
    kit.clock.advance(TimeDelta::hours(2));
    kit.microsoft.refuse_refresh(
        400,
        serde_json::json!({
            "error": "invalid_grant",
            "error_codes": [65001],
            "error_description": "AADSTS65001: The user or administrator has not consented."
        }),
    );
    kit.next_run().await;
    assert_eq!(connection(&kit, ws, &ana).await["health"], "needs_reauth");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn throttled_refreshes_pause_only_until_retry_after(db: PgPool) {
    let (kit, ws, ana) = connected(db).await;
    kit.clock.advance(TimeDelta::hours(2));
    kit.microsoft.throttle_refresh(120);
    kit.next_run().await;
    let throttled = connection(&kit, ws, &ana).await;
    assert_eq!(throttled["health"], "throttled");
    assert_eq!(due(&kit).await, kit.clock.now() + TimeDelta::seconds(120));
    kit.microsoft.allow_refresh();
    kit.next_run().await;
    assert_eq!(connection(&kit, ws, &ana).await["health"], "ok");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn without_notifications_the_mailbox_is_polled(db: PgPool) {
    let (kit, _, _) = connected(db).await;
    assert!(kit.microsoft.subscriptions().is_empty());
    assert!(
        !kit.microsoft
            .calls()
            .iter()
            .any(|c| c.contains("/subscriptions"))
    );
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM connectors.microsoft_subscriptions")
        .fetch_one(&kit.db)
        .await
        .unwrap();
    assert_eq!(rows, 0);
    // Adaptively: an empty mailbox's interval doubles from the shortest.
    let poll = &kit.deps.config.poll;
    let wait = due(&kit).await - kit.clock.now();
    assert!(
        wait < TimeDelta::seconds(i64::from(poll.max_seconds)),
        "{wait}"
    );
}

/// The ids of the folders `kit`'s stream reads.
async fn read_folders(kit: &Kit) -> Vec<String> {
    kit.stream().await["cursor"]["folders"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["id"].as_str().unwrap().to_owned())
        .collect()
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_folder_cap_keeps_what_it_reads_and_takes_nothing_for_gone(db: PgPool) {
    let mut config = Kit::microsoft_config();
    config.microsoft.max_folders = Some("6".into());
    let kit = Kit::with_config(db, config).await;
    let at = kit.clock.now() - TimeDelta::hours(1);
    let projects = kit.microsoft.add_folder("Projects", None);
    let filed = kit.microsoft.deliver(&projects, &synthetic(1), at);
    let (kit, ws, _) = connected_with(kit).await;
    // Every well-known folder is read whatever the cap, and Projects in the
    // one place left.
    let read = read_folders(&kit).await;
    for id in [
        "fld-inbox",
        "fld-sent",
        "fld-junk",
        "fld-deleted",
        "fld-archive",
        projects.as_str(),
    ] {
        assert!(read.iter().any(|r| r == id), "{id} in {read:?}");
    }
    // A new folder sorting first: Projects keeps its place, the new one
    // waits, and nothing is taken for gone.
    let first = kit.microsoft.add_folder("Aardvark", None);
    let waiting = kit.microsoft.deliver(&first, &synthetic(2), at);
    kit.clock.advance(crate::microsoft::read::FOLDERS_EVERY);
    kit.next_run().await;
    let read = read_folders(&kit).await;
    assert!(
        read.contains(&projects) && !read.contains(&first),
        "{read:?}"
    );
    let stream = kit.stream().await;
    assert_eq!(stream["cursor"]["partial"], true);
    assert_eq!(stream["cursor"]["generation"], 0, "no pass");
    // A pass now removes nothing it could not see.
    kit.microsoft.expire_delta("fld-inbox");
    kit.next_run().await;
    kit.run_jobs().await;
    let stream = kit.stream().await;
    assert_eq!(stream["cursor"]["generation"], 1);
    assert_eq!(stream["phase"], "incremental");
    let all = changes(&kit, ws).await;
    assert!(!removed(&all, &filed));
    assert_eq!(upserts(&all, &waiting), 0);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_first_round_at_graphs_limit_removes_nothing_it_left_out(db: PgPool) {
    let kit = Kit::with_filter_limit(db, Kit::microsoft_config(), Some(3)).await;
    let ids: Vec<String> = (1..=5_i64)
        .map(|n| {
            let at = kit.clock.now() - TimeDelta::hours(n);
            kit.microsoft
                .deliver("fld-inbox", &synthetic(usize::try_from(n).unwrap()), at)
        })
        .collect();
    let sent = kit.microsoft.deliver(
        "fld-sent",
        &synthetic(9),
        kit.clock.now() - TimeDelta::hours(1),
    );
    let (kit, ws, _) = connected_with(kit).await;
    // Graph lists the three newest only from now on; the Sent message is
    // deleted while Graph forgets the Inbox's delta.
    kit.microsoft.limit_filtered(Some(3));
    kit.microsoft.delete(&sent);
    kit.microsoft.expire_delta("fld-inbox");
    kit.next_run().await;
    kit.run_jobs().await;
    let stream = kit.stream().await;
    assert_eq!(stream["cursor"]["generation"], 1);
    let inbox = stream["cursor"]["folders"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["id"] == "fld-inbox")
        .unwrap()
        .clone();
    assert_eq!(inbox["truncated"], true);
    let all = changes(&kit, ws).await;
    // The two oldest were left out by Graph's limit, not deleted: kept.
    for id in &ids {
        assert!(!removed(&all, id), "{id}");
    }
    // Sent was listed whole: its deleted message is removed.
    assert!(removed(&all, &sent));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_well_known_folder_graph_does_not_name_once_keeps_its_role(db: PgPool) {
    let kit = Kit::with_microsoft(db).await;
    let at = kit.clock.now() - TimeDelta::hours(1);
    let archived = kit.microsoft.deliver("fld-archive", &synthetic(1), at);
    let (kit, ws, _) = connected_with(kit).await;
    kit.microsoft.hide_well_known("archive", true);
    kit.clock.advance(crate::microsoft::read::FOLDERS_EVERY);
    kit.next_run().await;
    let stream = kit.stream().await;
    let archive = stream["cursor"]["folders"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["id"] == "fld-archive")
        .unwrap()
        .clone();
    assert_eq!(archive["role"], "archive");
    assert_eq!(stream["cursor"]["generation"], 0, "no pass");
    assert_eq!(stream["cursor"]["partial"], true);
    assert!(!removed(&changes(&kit, ws).await, &archived));
    // Named again: the listing is whole again.
    kit.microsoft.hide_well_known("archive", false);
    kit.clock.advance(crate::microsoft::read::FOLDERS_EVERY);
    kit.next_run().await;
    assert!(kit.stream().await["cursor"].get("partial").is_none());
}
