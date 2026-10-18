//! The projection of `conversations.need_changed`.

use campbooks_kernel::tenancy::WorkspaceId;
use chrono::{TimeDelta, Utc};
use sqlx::PgPool;

use campbooks_kernel::db::BeginFor;

use crate::{
    domain::{ItemId, Reason, Trust},
    events::{ConversationNeedChanged, NeedFacts},
    testkit::{Kit, Need},
};

fn need(revision: i64, reason: Reason) -> Need {
    Need {
        conversation: "0192f0c4-7d4e-7b39-a7f1-3a8f0e2b9c11".into(),
        inbox: "0192f0c4-7d4e-7b39-a7f1-3a8f0e2b9c22".into(),
        message: "0192f0c4-7d4e-7b39-a7f1-3a8f0e2b9c33".into(),
        revision,
        reason,
        due_on: None,
        sent_at: Utc::now() - TimeDelta::hours(2),
        latest: None,
    }
}

async fn items(db: &PgPool) -> Vec<(String, String, i64)> {
    sqlx::query_as("SELECT reason, state, producer_revision FROM attention.items")
        .fetch_all(db)
        .await
        .unwrap()
}

#[sqlx::test(migrations = "../../../migrations")]
async fn one_item_per_conversation_and_newer_revisions_win(db: PgPool) {
    let kit = Kit::new(db.clone()).await;
    let workspace = WorkspaceId::new();
    kit.need(workspace, &need(1, Reason::Reply)).await;
    kit.need(workspace, &need(3, Reason::Pay)).await;
    // A late delivery of revision 2 changes nothing.
    kit.need(workspace, &need(2, Reason::Do)).await;
    assert_eq!(items(&db).await, [("pay".into(), "open".into(), 3)]);
    // The same event again (a replay) changes nothing either.
    kit.need(workspace, &need(3, Reason::Pay)).await;
    let changed = kit.events("attention.item_changed").await;
    assert_eq!(changed.len(), 2, "{changed:?}");
    assert_eq!(changed[0]["container"]["type"], "conversations.inbox");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn clearing_keeps_the_row_and_says_why(db: PgPool) {
    let kit = Kit::new(db.clone()).await;
    let workspace = WorkspaceId::new();
    kit.need(workspace, &need(1, Reason::Reply)).await;
    kit.cleared(workspace, &need(2, Reason::Reply), "replied", None)
        .await;
    let (state, cause): (String, String) =
        sqlx::query_as("SELECT state, cleared_because FROM attention.items")
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!((state.as_str(), cause.as_str()), ("cleared", "replied"));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn replaying_rebuilds_the_items(db: PgPool) {
    let kit = Kit::new(db.clone()).await;
    let workspace = WorkspaceId::new();
    kit.need(workspace, &need(1, Reason::Reply)).await;
    kit.need(workspace, &need(2, Reason::Do)).await;
    let before = items(&db).await;
    sqlx::query("DELETE FROM attention.items")
        .execute(&db)
        .await
        .unwrap();
    // Replay: deliver every logged need again, in log order.
    let logged = kit.events("conversations.need_changed").await;
    for payload in logged {
        let event: crate::events::ConversationNeedChanged =
            serde_json::from_value(payload).unwrap();
        let mut tx = campbooks_kernel::db::BeginFor::begin_for(&kit.state.db, workspace)
            .await
            .unwrap();
        kit.state.events.publish(&mut tx, &event).await.unwrap();
        tx.commit().await.unwrap();
    }
    kit.run_jobs().await;
    assert_eq!(items(&db).await, before);
}

/// `need` as `conversations.need_changed` carries it, open with `reason` or
/// cleared.
fn changed(workspace: WorkspaceId, need: &Need, open: bool) -> ConversationNeedChanged {
    ConversationNeedChanged {
        workspace: workspace.to_string(),
        inbox: need.inbox.clone(),
        conversation: need.conversation.clone(),
        message: need.message.clone(),
        revision: need.revision,
        sent_at: need.sent_at,
        need: open.then_some(NeedFacts {
            reason: need.reason,
            trust: Trust::Sure,
            due_on: None,
            urgent: false,
        }),
        cleared_because: (!open).then(|| "done".to_owned()),
        cleared_by: None,
        latest_message: None,
    }
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_older_delivery_losing_a_race_for_a_new_item_changes_nothing(db: PgPool) {
    // Two deliveries for a conversation with no item yet, run by two
    // workers at once: neither finds a row to lock, so both insert. The
    // newer one landed first; the older one, landing second, must not undo
    // it (campbooks-cloud#135).
    let kit = Kit::new(db.clone()).await;
    let workspace = WorkspaceId::new();
    let newer = changed(workspace, &need(2, Reason::Do), true);
    let facts = newer.need.clone().unwrap();
    let mut tx = BeginFor::begin_for(&kit.state.db, workspace).await.unwrap();
    let message = newer.message.clone();
    assert!(
        super::open(&mut tx, workspace, ItemId::new(), &newer, &facts, &message)
            .await
            .unwrap()
    );
    tx.commit().await.unwrap();
    // The older open, and an older clear, each with the id it made up.
    let older = changed(workspace, &need(1, Reason::Reply), true);
    let older_facts = older.need.clone().unwrap();
    let mut tx = BeginFor::begin_for(&kit.state.db, workspace).await.unwrap();
    assert!(
        !super::open(
            &mut tx,
            workspace,
            ItemId::new(),
            &older,
            &older_facts,
            &message
        )
        .await
        .unwrap()
    );
    let cleared = changed(workspace, &need(1, Reason::Reply), false);
    assert!(
        !super::clear(&mut tx, workspace, ItemId::new(), &cleared, &message)
            .await
            .unwrap()
    );
    tx.commit().await.unwrap();
    assert_eq!(items(&db).await, [("do".into(), "open".into(), 2)]);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_clear_delivered_before_its_open_wins(db: PgPool) {
    let kit = Kit::new(db.clone()).await;
    let workspace = WorkspaceId::new();
    // The reply clears it (revision 2) before the open (revision 1) arrives.
    kit.cleared(workspace, &need(2, Reason::Reply), "replied", None)
        .await;
    kit.need(workspace, &need(1, Reason::Reply)).await;
    let (state, reason, revision): (String, Option<String>, i64) =
        sqlx::query_as("SELECT state, reason, producer_revision FROM attention.items")
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!((state.as_str(), reason, revision), ("cleared", None, 2));
    // Nothing reached Home, nor Done today.
    assert!(kit.events("attention.item_changed").await.is_empty());
}

/// A tiny deterministic generator, so the shuffles below repeat.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self, bound: usize) -> usize {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1);
        usize::try_from(self.0 >> 33).unwrap_or(0) % bound
    }
}

#[sqlx::test(migrations = "../../../migrations")]
async fn any_delivery_order_ends_where_the_newest_revision_says(db: PgPool) {
    let kit = Kit::new(db.clone()).await;
    let workspace = WorkspaceId::new();
    let reasons = [Reason::Reply, Reason::Do, Reason::Pay, Reason::Review];
    for seed in 1_u64..=24 {
        let mut rng = Lcg(seed);
        let conversation = format!("0192f0c4-7d4e-7b39-a7f1-{seed:012}");
        // Revisions 1..=n, each an open (with some reason) or a clear.
        let n = 2 + rng.next(5);
        let mut history: Vec<(i64, Option<Reason>)> = (1..=n)
            .map(|revision| {
                let open = rng.next(3) != 0;
                let reason = reasons[rng.next(reasons.len())];
                (i64::try_from(revision).unwrap(), open.then_some(reason))
            })
            .collect();
        let expected = history.last().copied().unwrap();
        // Shuffle (Fisher–Yates), then deliver one by one.
        for i in (1..history.len()).rev() {
            history.swap(i, rng.next(i + 1));
        }
        for (revision, reason) in &history {
            let mut event = need(*revision, reason.unwrap_or(Reason::Reply));
            event.conversation = conversation.clone();
            match reason {
                Some(_) => kit.need(workspace, &event).await,
                None => kit.cleared(workspace, &event, "done", None).await,
            };
        }
        let (state, reason, revision): (String, Option<String>, i64) = sqlx::query_as(
            "SELECT state, reason, producer_revision FROM attention.items WHERE record_id = $1",
        )
        .bind(&conversation)
        .fetch_one(&db)
        .await
        .unwrap();
        assert_eq!(revision, expected.0, "seed {seed}: {history:?}");
        match expected.1 {
            Some(want) => {
                assert_eq!(state, "open", "seed {seed}: {history:?}");
                assert_eq!(reason.as_deref(), Some(want.as_str()), "seed {seed}");
            }
            None => assert_eq!(state, "cleared", "seed {seed}: {history:?}"),
        }
    }
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_deleted_inbox_takes_its_items_along(db: PgPool) {
    let kit = Kit::new(db.clone()).await;
    let workspace = WorkspaceId::new();
    kit.need(workspace, &need(1, Reason::Reply)).await;
    let mut tx = campbooks_kernel::db::BeginFor::begin_for(&kit.state.db, workspace)
        .await
        .unwrap();
    let inbox = campbooks_kernel::RecordRef::new(
        "conversations.inbox",
        "0192f0c4-7d4e-7b39-a7f1-3a8f0e2b9c22",
    );
    let gone = crate::domain::purge_container(&mut tx, &inbox)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(gone, 1);
    assert!(items(&db).await.is_empty());
}
