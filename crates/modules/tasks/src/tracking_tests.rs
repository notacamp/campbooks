//! The module's tracked events (ADR 0020, campbooks-cloud#340), read back
//! from the outbox: one per change, with the actor, the operation and a
//! snapshot of ids, enums, dates and times, and nothing for a change that
//! changes nothing. No event holds a title, an address or an amount: every
//! such text in these tests says `Shop`, `example` or `EUR`.

use axum::http::StatusCode;
use campbooks_kernel::{
    tenancy::{UserId, WorkspaceId},
    testing::{self, TrackedRow},
};
use serde_json::{Value, json};
use sqlx::PgPool;

use crate::{
    inbound::{MessageRead, ReadAsk},
    testkit::{CONVERSATION, INBOX, Kit},
};

const CREATED: &str = "tasks.task_created";
const EDITED: &str = "tasks.task_edited";
const DUE_CHANGED: &str = "tasks.task_due_changed";
const PLANNED: &str = "tasks.task_planned";
const DONE: &str = "tasks.task_done";
const NOT_NEEDED: &str = "tasks.task_not_needed";
const REOPENED: &str = "tasks.task_reopened";
const REASSIGNED: &str = "tasks.task_reassigned";

const MESSAGE: &str = "0192f0c4-7d4e-7b39-a7f1-3a8f0e2b9e01";

async fn tracked(kit: &Kit, name: &str) -> Vec<TrackedRow> {
    testing::tracked_events(&kit.db, name).await
}

/// No title, address or amount anywhere in the body.
fn nothing_personal(event: &TrackedRow) {
    let body = event.body.to_string();
    for text in ["Shop", "example", "@", "EUR", "12850", "title"] {
        assert!(
            !body.contains(text),
            "{} holds personal data or content ({text}): {body}",
            event.name
        );
    }
}

/// `event` is version 1 of `name`, in `workspace`, from `sub`'s own request
/// to `operation`.
fn by_user(event: &TrackedRow, name: &str, workspace: WorkspaceId, sub: &str, operation: &str) {
    assert_eq!(event.name, name);
    assert_eq!(event.version, 1, "{name}");
    assert_eq!(event.workspace_id, Some(*workspace.as_uuid()), "{name}");
    assert_eq!(
        event.envelope["actor"],
        json!({"type": "user", "id": sub}),
        "{name}"
    );
    assert_eq!(event.envelope["operation"], operation, "{name}");
    assert_eq!(
        event.envelope["workspace_id"],
        workspace.to_string(),
        "{name}"
    );
    assert_eq!(
        event.envelope["plan_version"],
        campbooks_kernel::tracking::PLAN_VERSION
    );
    nothing_personal(event);
}

/// `event` is version 1 of `name`, in `workspace`, tracked by the handler
/// `kind`.
fn by_system(event: &TrackedRow, name: &str, workspace: WorkspaceId, kind: &str) {
    assert_eq!(event.name, name);
    assert_eq!(event.version, 1, "{name}");
    assert_eq!(event.workspace_id, Some(*workspace.as_uuid()), "{name}");
    assert_eq!(
        event.envelope["actor"],
        json!({"type": "system", "id": kind}),
        "{name}"
    );
    nothing_personal(event);
}

/// `POST {base}/{action}` with `body`; the task afterwards.
async fn act(kit: &Kit, token: &str, base: &str, action: &str, body: Value) -> Value {
    let done = kit
        .post(token, &format!("{base}/{action}"), Some(body))
        .await;
    assert_eq!(done.status, StatusCode::OK, "{action}: {:?}", done.json);
    done.json
}

/// `PATCH base` with `body`; the task afterwards.
async fn update(kit: &Kit, token: &str, base: &str, body: Value) -> Value {
    let updated = kit.patch(token, base, body).await;
    assert_eq!(updated.status, StatusCode::OK, "{:?}", updated.json);
    updated.json
}

#[sqlx::test(migrations = "../../../migrations")]
#[allow(clippy::too_many_lines)]
async fn a_task_is_tracked_from_made_to_closed(db: PgPool) {
    let kit = Kit::new(db).await.tracking();
    let ws = WorkspaceId::new();
    let (ana, ana_id) = kit.member("ana", ws);
    kit.let_see(INBOX, ana_id);
    let made = kit
        .add(
            &ana,
            ws,
            json!({
                "title": "Pay the Shop invoice",
                "verb": "pay",
                "due_on": "2026-11-12",
                "source": {"type": "conversations.conversation", "id": CONVERSATION},
                "source_sender": "billing@example.com",
            }),
        )
        .await;
    let id = made["id"].as_str().unwrap().to_owned();
    let base = format!("/api/v1/w/{ws}/tasks/{id}");

    let created = tracked(&kit, CREATED).await;
    assert_eq!(created.len(), 1);
    by_user(&created[0], CREATED, ws, "ana", "tasks.create");
    let task = &created[0].body["task"];
    assert_eq!(task["id"], id);
    assert_eq!(task["verb"], "pay");
    assert_eq!(task["origin"], "own");
    assert_eq!(task["state"], "open");
    assert_eq!(task["due_on"], "2026-11-12");
    assert_eq!(task["plan"], "none");
    assert_eq!(task["assignee"], ana_id.to_string());
    assert_eq!(task["created_by"], ana_id.to_string());
    assert!(task.get("handed_by").is_none());
    assert_eq!(
        task["source"],
        json!({"type": "conversations.conversation", "id": CONVERSATION})
    );
    assert_eq!(task["has_amount"], false);
    assert_eq!(task["revision"], 1);

    // A new title and due day: edited (the field, never its value) and due
    // changed (the day before).
    let after = update(
        &kit,
        &ana,
        &base,
        json!({"revision": 1, "title": "Pay the Shop invoice now", "verb": "pay",
               "due_on": "2026-11-14"}),
    )
    .await;
    let edited = tracked(&kit, EDITED).await;
    assert_eq!(edited.len(), 1);
    by_user(&edited[0], EDITED, ws, "ana", "tasks.update");
    assert_eq!(edited[0].body["fields"], json!(["title"]));
    assert!(edited[0].body.get("before_verb").is_none());
    let due = tracked(&kit, DUE_CHANGED).await;
    assert_eq!(due.len(), 1);
    by_user(&due[0], DUE_CHANGED, ws, "ana", "tasks.update");
    assert_eq!(due[0].body["before"], json!({"due_on": "2026-11-12"}));
    assert_eq!(due[0].body["task"]["due_on"], "2026-11-14");
    assert_eq!(due[0].body["task"]["revision"], 2);

    // The same values again track nothing.
    let same = update(
        &kit,
        &ana,
        &base,
        json!({"revision": after["revision"], "title": "Pay the Shop invoice now",
               "verb": "pay", "due_on": "2026-11-14"}),
    )
    .await;
    assert_eq!(tracked(&kit, EDITED).await.len(), 1);
    assert_eq!(tracked(&kit, DUE_CHANGED).await.len(), 1);

    // The verb alone: before_verb.
    let after = update(
        &kit,
        &ana,
        &base,
        json!({"revision": same["revision"], "title": "Pay the Shop invoice now",
               "verb": "do", "due_on": "2026-11-14"}),
    )
    .await;
    let edited = tracked(&kit, EDITED).await;
    assert_eq!(edited.len(), 2);
    assert_eq!(edited[1].body["fields"], json!(["verb"]));
    assert_eq!(edited[1].body["before_verb"], "pay");
    assert_eq!(edited[1].body["task"]["verb"], "do");
    assert_eq!(tracked(&kit, DUE_CHANGED).await.len(), 1);

    // Planned for a day, with the plan before; the same plan again tracks
    // nothing.
    let plan = json!({"plan": "date", "plan_on": "2026-11-13"});
    let mut body = plan.clone();
    body["revision"] = after["revision"].clone();
    let after = act(&kit, &ana, &base, "plan", body).await;
    let planned = tracked(&kit, PLANNED).await;
    assert_eq!(planned.len(), 1);
    by_user(&planned[0], PLANNED, ws, "ana", "tasks.plan");
    assert_eq!(planned[0].body["before"], json!({"plan": "none"}));
    assert_eq!(planned[0].body["task"]["plan"], "date");
    assert_eq!(planned[0].body["task"]["plan_on"], "2026-11-13");
    let mut body = plan;
    body["revision"] = after["revision"].clone();
    let after = act(&kit, &ana, &base, "plan", body).await;
    assert_eq!(tracked(&kit, PLANNED).await.len(), 1);

    // Done, reopened (from done), not needed.
    let after = act(
        &kit,
        &ana,
        &base,
        "done",
        json!({"revision": after["revision"]}),
    )
    .await;
    let done = tracked(&kit, DONE).await;
    assert_eq!(done.len(), 1);
    by_user(&done[0], DONE, ws, "ana", "tasks.done");
    assert_eq!(done[0].body["task"]["state"], "done");
    assert!(done[0].body["task"]["closed_at"].is_string());

    let after = act(
        &kit,
        &ana,
        &base,
        "reopen",
        json!({"revision": after["revision"]}),
    )
    .await;
    let reopened = tracked(&kit, REOPENED).await;
    assert_eq!(reopened.len(), 1);
    by_user(&reopened[0], REOPENED, ws, "ana", "tasks.reopen");
    assert_eq!(reopened[0].body["before"], "done");
    assert_eq!(reopened[0].body["task"]["state"], "open");
    assert!(reopened[0].body["task"].get("closed_at").is_none());

    act(
        &kit,
        &ana,
        &base,
        "not_needed",
        json!({"revision": after["revision"]}),
    )
    .await;
    let not_needed = tracked(&kit, NOT_NEEDED).await;
    assert_eq!(not_needed.len(), 1);
    by_user(&not_needed[0], NOT_NEEDED, ws, "ana", "tasks.not_needed");
    assert_eq!(not_needed[0].body["task"]["state"], "not_needed");

    // A refused change (closed) tracks nothing.
    let refused = kit
        .post(
            &ana,
            &format!("{base}/done"),
            Some(json!({"revision": after["revision"]})),
        )
        .await;
    assert_eq!(refused.status, StatusCode::CONFLICT);
    assert_eq!(tracked(&kit, DONE).await.len(), 1);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn handing_off_and_taking_back_are_tracked(db: PgPool) {
    let kit = Kit::new(db).await.tracking();
    let ws = WorkspaceId::new();
    let (ana, ana_id) = kit.member("ana", ws);
    let (joao, joao_id) = kit.member("joao", ws);
    let (_, maria_id) = kit.member("maria", ws);
    let made = kit
        .add(&ana, ws, json!({"title": "Book the Shop venue"}))
        .await;
    let base = format!("/api/v1/w/{ws}/tasks/{}", made["id"].as_str().unwrap());

    // Ana hands it to João.
    let after = act(
        &kit,
        &ana,
        &base,
        "hand_off",
        json!({"revision": made["revision"], "to": joao_id}),
    )
    .await;
    let events = tracked(&kit, REASSIGNED).await;
    assert_eq!(events.len(), 1);
    by_user(&events[0], REASSIGNED, ws, "ana", "tasks.hand_off");
    assert_eq!(events[0].body["reassignment"], "handed_off");
    assert_eq!(events[0].body["task"]["assignee"], joao_id.to_string());
    assert_eq!(events[0].body["task"]["handed_by"], ana_id.to_string());
    assert_eq!(events[0].body["previous_assignee"], ana_id.to_string());
    assert!(events[0].body.get("previous_handed_by").is_none());

    // João hands it on to Maria: who gave it to him is kept.
    act(
        &kit,
        &joao,
        &base,
        "hand_off",
        json!({"revision": after["revision"], "to": maria_id}),
    )
    .await;
    let events = tracked(&kit, REASSIGNED).await;
    assert_eq!(events.len(), 2);
    by_user(&events[1], REASSIGNED, ws, "joao", "tasks.hand_off");
    assert_eq!(events[1].body["task"]["assignee"], maria_id.to_string());
    assert_eq!(events[1].body["task"]["handed_by"], joao_id.to_string());
    assert_eq!(events[1].body["previous_assignee"], joao_id.to_string());
    assert_eq!(events[1].body["previous_handed_by"], ana_id.to_string());

    // João takes it back from Maria.
    let revision: i64 = sqlx::query_scalar("SELECT revision FROM tasks.tasks")
        .fetch_one(&kit.db)
        .await
        .unwrap();
    act(
        &kit,
        &joao,
        &base,
        "take_back",
        json!({"revision": revision}),
    )
    .await;
    let events = tracked(&kit, REASSIGNED).await;
    assert_eq!(events.len(), 3);
    by_user(&events[2], REASSIGNED, ws, "joao", "tasks.take_back");
    assert_eq!(events[2].body["reassignment"], "taken_back");
    assert_eq!(events[2].body["task"]["assignee"], joao_id.to_string());
    assert!(events[2].body["task"].get("handed_by").is_none());
    assert_eq!(events[2].body["previous_assignee"], maria_id.to_string());
    assert_eq!(events[2].body["previous_handed_by"], joao_id.to_string());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn scout_making_a_task_tracks_it_on_the_members_behalf(db: PgPool) {
    let kit = Kit::new(db).await.tracking();
    let ws = WorkspaceId::new();
    let (_, ana_id) = kit.member("ana", ws);
    kit.as_assistant(
        "ana",
        ws,
        ("tasks.create", json!({"title": "Call the Shop"})),
    )
    .await
    .unwrap();

    let created = tracked(&kit, CREATED).await;
    assert_eq!(created.len(), 1);
    let envelope = &created[0].envelope;
    assert_eq!(envelope["actor"], json!({"type": "assistant"}));
    assert_eq!(
        envelope["on_behalf_of"],
        json!({"type": "user", "id": "ana"})
    );
    assert_eq!(envelope["operation"], "tasks.create");
    assert_eq!(created[0].body["task"]["assignee"], ana_id.to_string());
    nothing_personal(&created[0]);
}

/// Deliver a read of `MESSAGE` in `layer` to `owner`'s inbox.
async fn deliver(
    kit: &Kit,
    workspace: WorkspaceId,
    owner: UserId,
    layer: &str,
    asks: Vec<ReadAsk>,
    texts: Value,
) {
    let blob = kit
        .state
        .blobs
        .put(
            &workspace.to_string(),
            serde_json::to_vec(&texts).unwrap(),
            "application/vnd.campbooks.read-texts+json; version=1",
        )
        .await
        .unwrap();
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
                owner: Some(owner),
                layer: layer.to_owned(),
                asks,
                texts: Some(blob),
            },
        )
        .await
        .unwrap();
    tx.commit().await.unwrap();
    kit.run_jobs().await;
}

fn pay_ask(layer: &str, due: Option<&str>) -> ReadAsk {
    ReadAsk {
        key: format!("{MESSAGE}:{layer}:ask:1"),
        verb: "pay".to_owned(),
        due: due.map(str::to_owned),
        same_as: None,
    }
}

fn pay_texts(layer: &str) -> Value {
    json!({"asks": [
        {"key": format!("{MESSAGE}:{layer}:ask:1"), "title": "Pay the Shop invoice",
         "amount": {"minor": 12_850, "currency": "EUR"}},
    ]})
}

#[sqlx::test(migrations = "../../../migrations")]
async fn scouts_asks_are_tracked_as_the_handler(db: PgPool) {
    let kit = Kit::new(db).await.tracking();
    let ws = WorkspaceId::new();
    let (_, ana_id) = kit.member("ana", ws);

    // The structured read makes a pay task, with no due date.
    deliver(
        &kit,
        ws,
        ana_id,
        "structured",
        vec![pay_ask("structured", None)],
        pay_texts("structured"),
    )
    .await;
    let created = tracked(&kit, CREATED).await;
    assert_eq!(created.len(), 1);
    by_system(&created[0], CREATED, ws, "tasks.add_read_asks");
    let task = &created[0].body["task"];
    assert_eq!(task["origin"], "ask");
    assert_eq!(task["verb"], "pay");
    assert_eq!(task["assignee"], ana_id.to_string());
    assert_eq!(task["has_amount"], true);
    assert_eq!(
        task["source"],
        json!({"type": "conversations.conversation", "id": CONVERSATION})
    );
    assert!(task.get("due_on").is_none());

    // The model's read of the same message repeats it with a due date: the
    // task only gets the date.
    deliver(
        &kit,
        ws,
        ana_id,
        "model",
        vec![pay_ask("model", Some("2026-11-20"))],
        pay_texts("model"),
    )
    .await;
    assert_eq!(tracked(&kit, CREATED).await.len(), 1);
    let due = tracked(&kit, DUE_CHANGED).await;
    assert_eq!(due.len(), 1);
    by_system(&due[0], DUE_CHANGED, ws, "tasks.add_read_asks");
    assert_eq!(due[0].body["before"], json!({}));
    assert_eq!(due[0].body["task"]["due_on"], "2026-11-20");
    assert_eq!(due[0].body["task"]["id"], task["id"]);

    // A repeated delivery changes nothing and tracks nothing.
    deliver(
        &kit,
        ws,
        ana_id,
        "model",
        vec![pay_ask("model", Some("2026-11-20"))],
        pay_texts("model"),
    )
    .await;
    assert_eq!(tracked(&kit, CREATED).await.len(), 1);
    assert_eq!(tracked(&kit, DUE_CHANGED).await.len(), 1);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn with_tracking_off_nothing_is_written(db: PgPool) {
    let kit = Kit::new(db).await;
    let ws = WorkspaceId::new();
    let (ana, _) = kit.member("ana", ws);
    kit.add(&ana, ws, json!({"title": "Call the Shop"})).await;
    assert!(tracked(&kit, CREATED).await.is_empty());
}
