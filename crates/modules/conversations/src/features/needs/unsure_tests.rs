//! A guess is never shown as sure (campbooks-cloud#372), end to end: Joao
//! asks Ana Costa something, copying Sam's inbox, and the model reads it as
//! asking Sam for a reply at 0.97.

use std::sync::Arc;

use axum::http::StatusCode;
use campbooks_kernel::tenancy::WorkspaceId;
use serde_json::{Value, json};
use sqlx::PgPool;

use crate::testkit::{ANA, JOAO, Kit, Scripted, joao_asks_ana, script, sorting_deps};

/// Sam's deps with a model that reads Joao's mail as asking for a reply.
fn replying_deps() -> crate::ConversationsDeps {
    sorting_deps().with_model(Arc::new(Scripted::model([(
        JOAO.1,
        script("primary", true, false, false, 0.0, false),
    )])))
}

async fn summary(kit: &Kit, token: &str, workspace: WorkspaceId) -> Value {
    let id = kit.conversation_of("joao-1").await;
    let list = kit
        .get(
            token,
            &format!("/api/v1/w/{workspace}/conversations?ids={id}"),
        )
        .await;
    assert_eq!(list.status, StatusCode::OK, "{:?}", list.json);
    list.json["conversations"][0].clone()
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_copy_of_mail_between_others_is_a_question_even_when_the_model_is_sure(db: PgPool) {
    let kit = Kit::with_deps(db, replying_deps()).await;
    let (workspace, _, sam) = joao_asks_ana(&kit).await;

    let joao = summary(&kit, &sam, workspace).await;
    let need = &joao["need"];
    assert_eq!(need["reason"], "review", "{need}");
    assert_eq!(need["trust"], "check");
    assert_eq!(need["question"], "needs_reply");
    // What the member may answer it with: the headers' people, the one
    // natural party, how sure the model was and from where it is sure.
    let unsure = &need["unsure"];
    assert_eq!(unsure["in_to"], false, "{unsure}");
    assert_eq!(unsure["others"][0]["address"], ANA.1);
    assert_eq!(unsure["party"]["address"], ANA.1);
    assert_eq!(unsure["confidence"], 97);
    assert_eq!(unsure["sure_bar"], 90);
    assert!(unsure["unanswered_days"].is_number(), "{unsure}");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn someone_else_has_the_move_files_two_corrections_and_waits_on_them(db: PgPool) {
    let kit = Kit::with_deps(db.clone(), replying_deps()).await;
    let (workspace, _, sam) = joao_asks_ana(&kit).await;
    let id = kit.conversation_of("joao-1").await;
    let base = format!("/api/v1/w/{workspace}/conversations/{id}/need");
    let seen = kit.message_of("joao-1").await;

    // Not a person on the email: refused, nothing filed.
    let stranger = kit
        .post(
            &sam,
            &format!("{base}/resolve"),
            Some(json!({"outcome": "not_needed", "message_id": seen,
                        "wait_on": "eve@elsewhere.example"})),
        )
        .await;
    assert_eq!(stranger.status, StatusCode::CONFLICT, "{:?}", stranger.json);
    assert_eq!(stranger.json["code"], "not_a_party");
    // Nor can any other answer carry a person.
    let done = kit
        .post(
            &sam,
            &format!("{base}/resolve"),
            Some(json!({"outcome": "done", "message_id": seen, "wait_on": ANA.1})),
        )
        .await;
    assert_eq!(done.status, StatusCode::CONFLICT, "{:?}", done.json);
    let filed: i64 = sqlx::query_scalar("SELECT count(*) FROM conversations.corrections")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(filed, 0);

    let waited = kit
        .post(
            &sam,
            &format!("{base}/resolve"),
            Some(json!({"outcome": "not_needed", "message_id": seen,
                        "wait_on": ANA.1.to_uppercase()})),
        )
        .await;
    assert_eq!(waited.status, StatusCode::OK, "{:?}", waited.json);
    let need = &waited.json["need"];
    assert_eq!(need["reason"], "waiting", "{need}");
    assert_eq!(need["trust"], "sure");
    assert_eq!(need["basis"], "member");
    assert_eq!(need["question"], "next_move");
    assert_eq!(need["waiting_on"]["address"], ANA.1);
    assert_eq!(need["waiting_on"]["name"], ANA.0);

    // `next_move` and `awaited` are what was corrected.
    let rows: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT question, from_value, to_value FROM conversations.corrections
          WHERE withdrawn_at IS NULL ORDER BY question",
    )
    .fetch_all(&db)
    .await
    .unwrap();
    assert_eq!(
        rows,
        [
            ("awaited".to_owned(), "none".to_owned(), "p1".to_owned()),
            (
                "next_move".to_owned(),
                "recipient".to_owned(),
                "someone_else".to_owned()
            ),
        ]
    );
    // It stays a wait when worked out again.
    let again = summary(&kit, &sam, workspace).await;
    assert_eq!(again["need"]["reason"], "waiting", "{again}");

    // Undo brings the question back and withdraws both corrections.
    let reopened = kit.post(&sam, &format!("{base}/reopen"), None).await;
    assert_eq!(reopened.status, StatusCode::OK, "{:?}", reopened.json);
    assert_eq!(reopened.json["need"]["reason"], "review");
    assert_eq!(reopened.json["need"]["trust"], "check");
    let live: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM conversations.corrections WHERE withdrawn_at IS NULL",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(live, 0);
}
