//! The asks of an email become tasks.

use campbooks_kernel::tenancy::{UserId, WorkspaceId};
use serde_json::{Value, json};
use sqlx::PgPool;

use crate::{
    inbound::{MessageRead, ReadAsk},
    testkit::{CONVERSATION, INBOX, Kit},
};

const MESSAGE: &str = "0192f0c4-7d4e-7b39-a7f1-3a8f0e2b9e01";
/// The sender's own sentence, as a read quotes it.
const SENTENCE: &str = "Could you confirm Thursday works for the walkthrough?";

fn ask(key: &str, verb: &str, due: Option<&str>, same_as: Option<&str>) -> ReadAsk {
    ReadAsk {
        key: format!("{MESSAGE}:{key}"),
        verb: verb.to_owned(),
        due: due.map(str::to_owned),
        same_as: same_as.map(str::to_owned),
        done_by: None,
    }
}

fn done_by(mut ask: ReadAsk, how: &str) -> ReadAsk {
    ask.done_by = Some(how.to_owned());
    ask
}

/// Deliver a read of `MESSAGE`; `texts` are `(key suffix, title, amount)`.
async fn deliver(
    kit: &Kit,
    workspace: WorkspaceId,
    owner: Option<UserId>,
    layer: &str,
    asks: Vec<ReadAsk>,
    texts: Option<Value>,
) {
    let blob = match texts {
        Some(texts) => Some(
            kit.state
                .blobs
                .put(
                    &workspace.to_string(),
                    serde_json::to_vec(&texts).unwrap(),
                    "application/vnd.campbooks.read-texts+json; version=1",
                )
                .await
                .unwrap(),
        ),
        None => None,
    };
    let mut tx = kit.begin(workspace).await;
    kit.state
        .events
        .publish(
            &mut tx,
            &MessageRead {
                workspace: workspace.to_string(),
                message: MESSAGE.to_owned(),
                conversation: CONVERSATION.to_owned(),
                inbox: INBOX.to_owned(),
                owner,
                layer: layer.to_owned(),
                asks,
                texts: blob,
            },
        )
        .await
        .unwrap();
    tx.commit().await.unwrap();
    kit.run_jobs().await;
}

type Row = (
    String,
    String,
    String,
    Option<String>,
    Option<i64>,
    Option<String>,
);

async fn rows(kit: &Kit, workspace: WorkspaceId) -> Vec<Row> {
    sqlx::query_as(
        "SELECT title, verb, origin, due_on::text, amount_minor, currency FROM tasks.tasks
          WHERE workspace_id = $1 ORDER BY created_at, id",
    )
    .bind(workspace.as_uuid())
    .fetch_all(&kit.db)
    .await
    .unwrap()
}

fn lease_texts() -> Value {
    json!({"asks": [
        {"key": format!("{MESSAGE}:model:ask:1"), "title": "Send the signed lease"},
        {"key": format!("{MESSAGE}:model:ask:2"), "title": "Pay invoice 2026-114",
         "amount": {"minor": 12_850, "currency": "EUR"}},
    ]})
}

#[sqlx::test(migrations = "../../../migrations")]
async fn each_ask_becomes_a_task_of_the_owner(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (_, ana) = kit.member("ana", workspace);
    deliver(
        &kit,
        workspace,
        Some(ana),
        "model",
        vec![
            ask("model:ask:1", "do", Some("2026-10-30"), None),
            ask(
                "model:ask:2",
                "pay",
                Some("2026-11-02T10:00:00+01:00"),
                None,
            ),
        ],
        Some(lease_texts()),
    )
    .await;

    assert_eq!(
        rows(&kit, workspace).await,
        [
            (
                "Send the signed lease".to_owned(),
                "do".to_owned(),
                "ask".to_owned(),
                Some("2026-10-30".to_owned()),
                None,
                None
            ),
            (
                "Pay invoice 2026-114".to_owned(),
                "pay".to_owned(),
                "ask".to_owned(),
                Some("2026-11-02".to_owned()),
                Some(12_850),
                Some("EUR".to_owned())
            ),
        ]
    );
    let (assignee, source, container, actor): (UserId, String, String, String) = sqlx::query_as(
        "SELECT t.assignee, t.source_id, t.source_container, h.actor
           FROM tasks.tasks t JOIN tasks.history h ON h.task_id = t.id
          WHERE t.verb = 'do'",
    )
    .fetch_one(&kit.db)
    .await
    .unwrap();
    assert_eq!(
        (assignee, source.as_str(), container.as_str()),
        (ana, CONVERSATION, INBOX)
    );
    assert_eq!(actor, "scout");
    let events = kit.events("tasks.task_changed").await;
    assert_eq!(events.len(), 2);
    assert_eq!(events[0]["origin"], "ask");
    assert!(events[0].get("title").is_none());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_repeated_delivery_changes_nothing(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana_token, ana) = kit.member("ana", workspace);
    let asks = || vec![ask("model:ask:1", "do", Some("2026-10-30"), None)];
    deliver(
        &kit,
        workspace,
        Some(ana),
        "model",
        asks(),
        Some(lease_texts()),
    )
    .await;
    // Ana closes it; a replay must not reopen or duplicate it.
    let id: String = sqlx::query_scalar("SELECT id::text FROM tasks.tasks")
        .fetch_one(&kit.db)
        .await
        .unwrap();
    let done = kit
        .post(
            &ana_token,
            &format!("/api/v1/w/{workspace}/tasks/{id}/done"),
            Some(json!({"revision": 1})),
        )
        .await;
    assert_eq!(done.json["state"], "done");
    deliver(
        &kit,
        workspace,
        Some(ana),
        "model",
        asks(),
        Some(lease_texts()),
    )
    .await;
    assert_eq!(kit.task_count(workspace).await, 1);
    let state: String = sqlx::query_scalar("SELECT state FROM tasks.tasks")
        .fetch_one(&kit.db)
        .await
        .unwrap();
    assert_eq!(state, "done");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_shared_inbox_makes_none(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    deliver(
        &kit,
        workspace,
        None,
        "model",
        vec![ask("model:ask:2", "pay", None, None)],
        Some(lease_texts()),
    )
    .await;
    assert_eq!(kit.task_count(workspace).await, 0);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_ask_without_words_gets_the_verbs_own(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (_, ana) = kit.member("ana", workspace);
    deliver(
        &kit,
        workspace,
        Some(ana),
        "model",
        vec![ask("model:ask:1", "pay", None, None)],
        None,
    )
    .await;
    let found = rows(&kit, workspace).await;
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].0, "Pay what this email asks");
    assert_eq!(found[0].3, None);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_ask_that_is_an_open_task_only_fills_its_missing_date(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (token, ana) = kit.member("ana", workspace);
    kit.let_see(INBOX, ana);
    let source = json!({"type": "conversations.conversation", "id": CONVERSATION});
    let undated = kit
        .add(
            &token,
            workspace,
            json!({"title": "Pay EDP", "source": source}),
        )
        .await;
    let dated = kit
        .add(
            &token,
            workspace,
            json!({"title": "Pay the water", "due_on": "2026-10-28", "source": source}),
        )
        .await;
    let (undated, dated) = (
        undated["id"].as_str().unwrap(),
        dated["id"].as_str().unwrap(),
    );

    deliver(
        &kit,
        workspace,
        Some(ana),
        "model",
        vec![
            ask("model:ask:1", "pay", Some("2026-10-30"), Some(undated)),
            ask("model:ask:2", "pay", Some("2026-10-30"), Some(dated)),
        ],
        Some(lease_texts()),
    )
    .await;

    let due: Vec<(String, Option<String>)> =
        sqlx::query_as("SELECT title, due_on::text FROM tasks.tasks ORDER BY created_at, id")
            .fetch_all(&kit.db)
            .await
            .unwrap();
    assert_eq!(
        due,
        [
            ("Pay EDP".to_owned(), Some("2026-10-30".to_owned())),
            ("Pay the water".to_owned(), Some("2026-10-28".to_owned())),
        ]
    );
    // Both were the open tasks: nothing was added.
    assert_eq!(kit.task_count(workspace).await, 2);
    assert_eq!(kit.events("tasks.task_changed").await.len(), 3);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_same_as_that_is_not_the_owners_open_task_is_ignored(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let other_workspace = WorkspaceId::new();
    let (ana_token, ana) = kit.member("ana", workspace);
    let (joao_token, joao) = kit.member("joao", workspace);
    let (bo_token, _) = kit.member("bo", other_workspace);
    for user in [ana, joao] {
        kit.let_see(INBOX, user);
    }
    let source = json!({"type": "conversations.conversation", "id": CONVERSATION});
    // João's task of the same conversation, Ana's closed one, another
    // workspace's, and Ana's own from elsewhere.
    let joaos = kit
        .add(
            &joao_token,
            workspace,
            json!({"title": "João's", "source": source}),
        )
        .await["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let closed = kit
        .add(
            &ana_token,
            workspace,
            json!({"title": "Closed", "source": source}),
        )
        .await["id"]
        .as_str()
        .unwrap()
        .to_owned();
    kit.post(
        &ana_token,
        &format!("/api/v1/w/{workspace}/tasks/{closed}/not_needed"),
        Some(json!({"revision": 1})),
    )
    .await;
    let foreign = kit
        .add(&bo_token, other_workspace, json!({"title": "Elsewhere"}))
        .await["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let own = kit
        .add(&ana_token, workspace, json!({"title": "Unrelated"}))
        .await["id"]
        .as_str()
        .unwrap()
        .to_owned();

    let asks = [joaos, closed, foreign, own, "not-an-id".to_owned()]
        .iter()
        .enumerate()
        .map(|(n, same)| {
            ask(
                &format!("model:ask:{}", n + 1),
                "do",
                Some("2026-10-30"),
                Some(same),
            )
        })
        .collect();
    deliver(&kit, workspace, Some(ana), "model", asks, None).await;

    // Each was a new ask for Ana; nobody else's task got a date.
    let ana_new: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM tasks.tasks WHERE origin = 'ask' AND assignee = $1",
    )
    .bind(ana.as_uuid())
    .fetch_one(&kit.db)
    .await
    .unwrap();
    assert_eq!(ana_new, 5);
    let dated: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM tasks.tasks WHERE origin = 'own' AND due_on IS NOT NULL",
    )
    .fetch_one(&kit.db)
    .await
    .unwrap();
    assert_eq!(dated, 0);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_two_layers_of_one_invoice_make_one_pay_task(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (_, ana) = kit.member("ana", workspace);
    let structured_texts = json!({"asks": [
        {"key": format!("{MESSAGE}:structured:ask:1"), "title": "Pay invoice 2026-114",
         "amount": {"minor": 12_850, "currency": "EUR"}},
    ]});
    deliver(
        &kit,
        workspace,
        Some(ana),
        "structured",
        vec![ask("structured:ask:1", "pay", None, None)],
        Some(structured_texts),
    )
    .await;
    // The model read does not name it (same_as absent) but dates it.
    deliver(
        &kit,
        workspace,
        Some(ana),
        "model",
        vec![ask("model:ask:1", "pay", Some("2026-10-30"), None)],
        Some(json!({"asks": [
            {"key": format!("{MESSAGE}:model:ask:1"), "title": "Pay EDP by 30 October"},
        ]})),
    )
    .await;
    let found = rows(&kit, workspace).await;
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].0, "Pay invoice 2026-114");
    assert_eq!(found[0].3.as_deref(), Some("2026-10-30"));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_model_read_that_lands_first_is_not_doubled_by_the_structured_read(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (_, ana) = kit.member("ana", workspace);
    deliver(
        &kit,
        workspace,
        Some(ana),
        "model",
        vec![ask("model:ask:1", "pay", Some("2026-10-30"), None)],
        Some(json!({"asks": [
            {"key": format!("{MESSAGE}:model:ask:1"), "title": "Pay EDP by 30 October"},
        ]})),
    )
    .await;
    deliver(
        &kit,
        workspace,
        Some(ana),
        "structured",
        vec![ask("structured:ask:1", "pay", Some("2026-11-05"), None)],
        Some(json!({"asks": [
            {"key": format!("{MESSAGE}:structured:ask:1"), "title": "Pay invoice 2026-114"},
        ]})),
    )
    .await;
    let found = rows(&kit, workspace).await;
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].0, "Pay EDP by 30 October");
    // The date it had stays.
    assert_eq!(found[0].3.as_deref(), Some("2026-10-30"));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn how_an_ask_gets_done_is_kept_and_a_payment_is_always_elsewhere(db: PgPool) {
    let kit = Kit::new(db).await.tracking();
    let workspace = WorkspaceId::new();
    let (ana_token, ana) = kit.member("ana", workspace);
    // The sender's sentence rides along as a read could carry it; the tasks
    // module must keep none of it, so the checks below look for these words.
    let texts = json!({"asks": [
        {"key": format!("{MESSAGE}:model:ask:1"), "title": "Confirm Thursday", "quote": SENTENCE},
        {"key": format!("{MESSAGE}:model:ask:2"), "title": "Sign the lease"},
        {"key": format!("{MESSAGE}:model:ask:3"), "title": "Pay the deposit"},
    ]});
    deliver(
        &kit,
        workspace,
        Some(ana),
        "model",
        vec![
            done_by(ask("model:ask:1", "do", None, None), "reply"),
            done_by(ask("model:ask:2", "do", None, None), "elsewhere"),
            // A payment the read called a reply is still done elsewhere.
            done_by(ask("model:ask:3", "pay", None, None), "reply"),
        ],
        Some(texts),
    )
    .await;
    let kept: Vec<(String, Option<String>)> =
        sqlx::query_as("SELECT title, done_by FROM tasks.tasks ORDER BY title")
            .fetch_all(&kit.db)
            .await
            .unwrap();
    assert_eq!(
        kept,
        [
            ("Confirm Thursday".to_owned(), Some("reply".to_owned())),
            ("Pay the deposit".to_owned(), Some("elsewhere".to_owned())),
            ("Sign the lease".to_owned(), Some("elsewhere".to_owned())),
        ]
    );
    // The API says how, and names the ask by key (ids only); it carries no
    // sentence of the sender's, nor do the events or the tracked events.
    let list = kit
        .get(&ana_token, &format!("/api/v1/w/{workspace}/tasks"))
        .await
        .json
        .to_string();
    assert!(list.contains(r#""done_by":"reply""#), "{list}");
    assert!(list.contains(&format!("{MESSAGE}:model:ask:1")), "{list}");
    assert!(!list.contains(SENTENCE), "{list}");
    let rows: Vec<String> = sqlx::query_scalar("SELECT row_to_json(t)::text FROM tasks.tasks t")
        .fetch_all(&kit.db)
        .await
        .unwrap();
    assert!(rows.iter().all(|row| !row.contains(SENTENCE)), "{rows:?}");
    let events = kit.events("tasks.task_changed").await;
    assert_eq!(events.len(), 3);
    assert!(
        events
            .iter()
            .all(|event| !event.to_string().contains(SENTENCE))
    );
    assert!(
        campbooks_kernel::testing::tracked_events(&kit.db, "tasks.task_created")
            .await
            .iter()
            .all(|event| !event.body.to_string().contains(SENTENCE))
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_read_from_before_it_was_asked_leaves_how_it_gets_done_open(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (_, ana) = kit.member("ana", workspace);
    deliver(
        &kit,
        workspace,
        Some(ana),
        "model",
        vec![ask("model:ask:1", "do", None, None)],
        Some(lease_texts()),
    )
    .await;
    let how: Option<String> = sqlx::query_scalar("SELECT done_by FROM tasks.tasks")
        .fetch_one(&kit.db)
        .await
        .unwrap();
    assert_eq!(how, None);
}
