//! Teach once from Scout's "whose move is it?" answers (campbooks-cloud#378),
//! end to end: Joao writes to Ana Costa, copying Sam's inbox, and the model
//! reads it as asking Sam for a reply, so Scout asks.

use std::sync::Arc;

use axum::http::StatusCode;
use campbooks_kernel::tenancy::WorkspaceId;
use serde_json::{Value, json};
use sqlx::PgPool;

use crate::{
    ConversationsDeps,
    model::InboxId,
    testkit::{ANA, JOAO, Kit, SAM, Scripted, joao_asks_ana, mail, script, sorting_deps},
};

/// Sam's deps with whose move it is switched on and a model that reads
/// Joao's mail as asking for a reply.
fn replying_deps() -> ConversationsDeps {
    let mut deps = sorting_deps().with_model(Arc::new(Scripted::model([(
        JOAO.1,
        script("primary", true, false, false, 0.0, false),
    )])));
    deps.config.moves.enabled = true;
    deps
}

/// Joao's second message to Ana, in a thread of its own, copying Sam.
async fn joao_again(kit: &Kit, (workspace, inbox): (WorkspaceId, InboxId), id: &str) {
    kit.ingest_value(
        workspace,
        inbox,
        &mail(id, &format!("thr-{id}"), JOAO, &[ANA], &[SAM], 30),
    )
    .await;
}

/// Sam answers "Nudge Ana" on `message`'s conversation.
async fn nudge(kit: &Kit, sam: &str, workspace: WorkspaceId, message: &str) -> Value {
    let conversation = kit.conversation_of(message).await;
    let seen = kit.message_of(message).await;
    let answered = kit
        .post(
            sam,
            &format!("/api/v1/w/{workspace}/conversations/{conversation}/need/resolve"),
            Some(json!({"outcome": "not_needed", "message_id": seen, "wait_on": ANA.1})),
        )
        .await;
    assert_eq!(answered.status, StatusCode::OK, "{:?}", answered.json);
    answered.json
}

async fn offer(kit: &Kit, sam: &str, workspace: WorkspaceId, message: &str) -> Value {
    let conversation = kit.conversation_of(message).await;
    let offered = kit
        .get(
            sam,
            &format!(
                "/api/v1/w/{workspace}/conversations/{conversation}/rule-suggestions?action=waiting"
            ),
        )
        .await;
    assert_eq!(offered.status, StatusCode::OK, "{:?}", offered.json);
    offered.json
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_second_nudge_to_the_same_sender_offers_the_rule_once(db: PgPool) {
    let kit = Kit::with_deps(db, replying_deps()).await;
    let (workspace, inbox, sam) = joao_asks_ana(&kit).await;
    joao_again(&kit, (workspace, inbox), "joao-2").await;

    // Once is an answer, not a habit.
    nudge(&kit, &sam, workspace, "joao-1").await;
    let first = offer(&kit, &sam, workspace, "joao-1").await;
    assert_eq!(first["none"], "too_few", "{first}");
    assert!(first["suggestions"].as_array().unwrap().is_empty());

    // The same answer for the same sender, a second time.
    nudge(&kit, &sam, workspace, "joao-2").await;
    let second = offer(&kit, &sam, workspace, "joao-2").await;
    assert!(second["none"].is_null(), "{second}");
    assert_eq!(second["default"], 0);
    assert_eq!(second["wait_days"], 3, "the usual working days");
    let rule = &second["suggestions"][0];
    assert_eq!(
        rule["conditions"],
        json!([
            {"field": "sender_address", "value": JOAO.1},
            {"field": "member_not_in_to"}
        ])
    );
    assert_eq!(rule["actions"], json!([{"action": "wait_on_addressed"}]));

    // Undo of the answer cancels the offer: only one answer is left.
    let conversation = kit.conversation_of("joao-2").await;
    let reopened = kit
        .post(
            &sam,
            &format!("/api/v1/w/{workspace}/conversations/{conversation}/need/reopen"),
            None,
        )
        .await;
    assert_eq!(reopened.status, StatusCode::OK, "{:?}", reopened.json);
    let undone = offer(&kit, &sam, workspace, "joao-2").await;
    assert_eq!(undone["none"], "not_teachable", "{undone}");
    nudge(&kit, &sam, workspace, "joao-2").await;

    // "Ask me each time": not offered again.
    let again = offer(&kit, &sam, workspace, "joao-2").await;
    let key = again["suggestions"][0]["key"].as_str().unwrap();
    let dismissed = kit
        .post(
            &sam,
            &format!("/api/v1/w/{workspace}/conversations/{conversation}/rule-suggestions/dismiss"),
            Some(json!({"key": key})),
        )
        .await;
    assert_eq!(dismissed.status, StatusCode::NO_CONTENT);
    let gone = offer(&kit, &sam, workspace, "joao-2").await;
    assert_eq!(gone["none"], "dismissed", "{gone}");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_nudge_to_someone_the_rule_would_not_name_teaches_nothing(db: PgPool) {
    let kit = Kit::with_deps(db, replying_deps()).await;
    let (workspace, inbox, sam) = joao_asks_ana(&kit).await;
    joao_again(&kit, (workspace, inbox), "joao-2").await;
    nudge(&kit, &sam, workspace, "joao-1").await;
    nudge(&kit, &sam, workspace, "joao-2").await;
    // Joao's mail to someone else, with the same two answers behind it, is
    // not offered when the member was not only copied: here Sam is in To.
    kit.ingest_value(
        workspace,
        inbox,
        &mail("joao-3", "thr-joao-3", JOAO, &[ANA, SAM], &[], 20),
    )
    .await;
    let offered = offer(&kit, &sam, workspace, "joao-3").await;
    assert_eq!(offered["none"], "not_teachable", "{offered}");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_rule_waits_on_the_person_it_was_written_to_and_sends_nothing(db: PgPool) {
    let kit = Kit::with_deps(db, replying_deps()).await;
    let workspace = WorkspaceId::new();
    let (sam, sam_id) = kit.member("sam", workspace);
    let inbox = kit
        .inbox(
            workspace,
            SAM.1,
            &[(sam_id, crate::model::InboxRole::Manager)],
        )
        .await;
    let created = kit
        .post(
            &sam,
            &format!("/api/v1/w/{workspace}/inboxes/{inbox}/rules"),
            Some(json!({
                "conditions": [
                    {"field": "sender_address", "value": JOAO.1},
                    {"field": "member_not_in_to"}
                ],
                "actions": [{"action": "wait_on_addressed"}],
            })),
        )
        .await;
    assert_eq!(created.status, StatusCode::CREATED, "{:?}", created.json);

    // Written to Ana, Sam in Cc: the conversation waits on Ana at once, and
    // the model's ask is nothing (the member's rule speaks for them).
    kit.ingest_value(
        workspace,
        inbox,
        &mail("joao-1", "thr-floor-plan", JOAO, &[ANA], &[SAM], 60),
    )
    .await;
    let waiting = need_of(&kit, "joao-1").await;
    assert_eq!(
        waiting,
        Some((
            "waiting".to_owned(),
            "sure".to_owned(),
            "taught".to_owned(),
            Some(ANA.1.to_owned())
        ))
    );

    // Written to Sam: the rule does not apply, Scout asks as before.
    kit.ingest_value(
        workspace,
        inbox,
        &mail("joao-2", "thr-joao-2", JOAO, &[SAM], &[], 30),
    )
    .await;
    let asked = need_of(&kit, "joao-2").await.unwrap();
    assert_eq!(asked.0, "reply", "{asked:?}");

    // Written to two people besides Sam: no single obvious person, so the
    // rule answers nothing and the card asks.
    kit.ingest_value(
        workspace,
        inbox,
        &mail(
            "joao-3",
            "thr-joao-3",
            JOAO,
            &[ANA, ("Eva Lopes", "eva@studio.example")],
            &[SAM],
            20,
        ),
    )
    .await;
    let unsure = need_of(&kit, "joao-3").await.unwrap();
    assert_eq!(unsure.0, "review", "{unsure:?}");

    // Nothing was sent on the rule's behalf.
    let sent: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM conversations.messages WHERE direction = 'outbound'",
    )
    .fetch_one(&kit.db)
    .await
    .unwrap();
    assert_eq!(sent, 0);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_reply_the_member_taught_comes_before_the_wait(db: PgPool) {
    let kit = Kit::with_deps(db, replying_deps()).await;
    let workspace = WorkspaceId::new();
    let (sam, sam_id) = kit.member("sam", workspace);
    let inbox = kit
        .inbox(
            workspace,
            SAM.1,
            &[(sam_id, crate::model::InboxRole::Manager)],
        )
        .await;
    for (conditions, actions) in [
        (
            json!([
                {"field": "sender_address", "value": JOAO.1},
                {"field": "member_not_in_to"}
            ]),
            json!([{"action": "wait_on_addressed"}]),
        ),
        (
            json!([{"field": "sender_address", "value": JOAO.1}]),
            json!([{"action": "needs_reply", "value": true}]),
        ),
    ] {
        let created = kit
            .post(
                &sam,
                &format!("/api/v1/w/{workspace}/inboxes/{inbox}/rules"),
                Some(json!({"conditions": conditions, "actions": actions})),
            )
            .await;
        assert_eq!(created.status, StatusCode::CREATED, "{:?}", created.json);
    }
    kit.ingest_value(
        workspace,
        inbox,
        &mail("joao-1", "thr-floor-plan", JOAO, &[ANA], &[SAM], 60),
    )
    .await;
    // The member's own "needs a reply" is never silently overridden: it
    // raises its Reply, and the wait, which only fills an empty place, is
    // not raised.
    let need = need_of(&kit, "joao-1").await.unwrap();
    assert_eq!(
        (need.0.as_str(), need.2.as_str()),
        ("reply", "taught"),
        "{need:?}"
    );
}

/// `(reason, trust, basis, awaited address)` of the open need of the
/// conversation holding `message`.
async fn need_of(kit: &Kit, message: &str) -> Option<(String, String, String, Option<String>)> {
    let conversation = kit.conversation_of(message).await;
    sqlx::query_as(
        "SELECT reason, trust, basis, awaited_address FROM conversations.needs
          WHERE conversation_id = $1::uuid AND state = 'open'",
    )
    .bind(&conversation)
    .fetch_optional(&kit.db)
    .await
    .unwrap()
}
