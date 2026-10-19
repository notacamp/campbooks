use axum::http::StatusCode;
use serde_json::{Value, json};
use sqlx::PgPool;

use crate::{model::InboxRole, testkit::Kit};

fn addresses(list: &Value) -> Vec<&str> {
    list.as_array()
        .unwrap()
        .iter()
        .map(|a| a["address"].as_str().unwrap())
        .collect()
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_reply_is_filled_from_the_message(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, inbox, sam) = kit.sams_inbox().await;
    let message = kit.message_of("en-1").await;
    let created = kit
        .post(
            &sam,
            &format!("/api/v1/w/{workspace}/drafts"),
            Some(json!({"kind": "reply", "message_id": message})),
        )
        .await;
    assert_eq!(created.status, StatusCode::CREATED, "{:?}", created.json);
    let draft = &created.json;
    assert_eq!(draft["inbox_id"], inbox.to_string());
    assert_eq!(draft["kind"], "reply");
    assert_eq!(draft["from"]["address"], "sam@example.com");
    assert_eq!(addresses(&draft["to"]), ["maya@northwind-studio.example"]);
    assert!(addresses(&draft["cc"]).is_empty());
    assert_eq!(draft["subject"], "Re: Q4 plan: can you review by Friday?");
    assert_eq!(draft["state"], "draft");
    assert_eq!(draft["version"], 1);
    assert_eq!(draft["conversation_id"], kit.conversation_of("en-1").await);
    let row: (Option<String>, Vec<String>, Option<String>) =
        sqlx::query_as("SELECT in_reply_to, reference_ids, thread_key FROM conversations.drafts")
            .fetch_one(&kit.db)
            .await
            .unwrap();
    assert_eq!(row.0.as_deref(), Some("q4-plan-1@studio.example"));
    assert_eq!(row.1, ["q4-plan-1@studio.example"]);
    assert_eq!(row.2.as_deref(), Some("thr-en-q4"));
    // Saving publishes nothing.
    assert!(kit.events("conversations.send_requested").await.is_empty());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn reply_all_leaves_the_inbox_out(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, _, sam) = kit.sams_inbox().await;
    let uri = format!("/api/v1/w/{workspace}/drafts");
    let inbound = kit.message_of("en-1").await;
    let all = kit
        .post(
            &sam,
            &uri,
            Some(json!({"kind": "reply_all", "message_id": inbound})),
        )
        .await;
    assert_eq!(
        addresses(&all.json["to"]),
        ["maya@northwind-studio.example"]
    );
    assert_eq!(addresses(&all.json["cc"]), ["leo@northwind-studio.example"]);
    // A reply to Sam's own message goes to whom Sam wrote, and the subject
    // keeps its one `Re:`.
    let own = kit.message_of("en-2").await;
    let reply = kit
        .post(
            &sam,
            &uri,
            Some(json!({"kind": "reply", "message_id": own})),
        )
        .await;
    assert_eq!(
        addresses(&reply.json["to"]),
        ["maya@northwind-studio.example"]
    );
    assert_eq!(
        reply.json["subject"],
        "Re: Q4 plan: can you review by Friday?"
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_forward_holds_the_attachments(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, _, sam) = kit.sams_inbox().await;
    let message = kit.message_of("en-1").await;
    let created = kit
        .post(
            &sam,
            &format!("/api/v1/w/{workspace}/drafts"),
            Some(json!({"kind": "forward", "message_id": message})),
        )
        .await;
    assert_eq!(created.status, StatusCode::CREATED, "{:?}", created.json);
    let draft = &created.json;
    assert!(addresses(&draft["to"]).is_empty());
    assert_eq!(draft["subject"], "Fwd: Q4 plan: can you review by Friday?");
    assert_eq!(draft["attachments"][0]["filename"], "q4-plan.pdf");
    assert!(draft["attachments"][0].get("blob").is_none());
    let held: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM kernel.blob_refs WHERE owner_type = 'conversations.draft'",
    )
    .fetch_one(&kit.db)
    .await
    .unwrap();
    assert_eq!(held, 1);

    // Dropping it in the composer releases it.
    let id = draft["id"].as_str().unwrap();
    let saved = kit
        .patch(
            &sam,
            &format!("/api/v1/w/{workspace}/drafts/{id}"),
            json!({"version": 1, "attachments": [], "content_token": draft["content_token"]}),
        )
        .await;
    assert_eq!(saved.status, StatusCode::OK, "{:?}", saved.json);
    assert_eq!(saved.json["attachments"], json!([]));
    let held: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM kernel.blob_refs WHERE owner_type = 'conversations.draft'",
    )
    .fetch_one(&kit.db)
    .await
    .unwrap();
    assert_eq!(held, 0);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn readers_cannot_write_and_drafts_are_private(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, inbox, sam) = kit.sams_inbox().await;
    let uri = format!("/api/v1/w/{workspace}/drafts");
    let message = kit.message_of("en-1").await;
    let (ana, ana_id) = kit.member("ana", workspace);
    // Not a member of the inbox: the message is not there for Ana.
    let response = kit
        .post(
            &ana,
            &uri,
            Some(json!({"kind": "reply", "message_id": message})),
        )
        .await;
    assert_eq!(response.status, StatusCode::NOT_FOUND);
    // A reader may not write.
    kit.add_to_inbox(inbox, ana_id, InboxRole::Reader).await;
    let response = kit
        .post(&ana, &uri, Some(json!({"kind": "new", "inbox_id": inbox})))
        .await;
    assert_eq!(response.status, StatusCode::FORBIDDEN);
    assert_eq!(response.json["code"], "inbox_read_only");

    // Sam's draft is Sam's alone, even for another sender of the inbox.
    let (leo, leo_id) = kit.member("leo", workspace);
    kit.add_to_inbox(inbox, leo_id, InboxRole::Sender).await;
    let draft = kit
        .post(&sam, &uri, Some(json!({"kind": "new", "inbox_id": inbox})))
        .await;
    let id = draft.json["id"].as_str().unwrap();
    let one = format!("{uri}/{id}");
    assert_eq!(kit.get(&leo, &one).await.status, StatusCode::NOT_FOUND);
    assert_eq!(kit.get(&sam, &one).await.status, StatusCode::OK);
    assert_eq!(kit.get(&leo, &uri).await.json["drafts"], json!([]));
    assert_eq!(
        kit.get(&sam, &uri).await.json["drafts"][0]["id"],
        id.to_owned()
    );
    let client = kit.client(workspace, "conversations:send");
    assert_eq!(kit.get(&client, &one).await.status, StatusCode::NOT_FOUND);

    // Without the scope.
    let tidy_only = kit.client(workspace, "conversations:read conversations:write");
    let response = kit
        .post(
            &tidy_only,
            &uri,
            Some(json!({"kind": "new", "inbox_id": inbox})),
        )
        .await;
    assert_eq!(response.status, StatusCode::FORBIDDEN);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn saving_needs_the_current_version(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, inbox, sam) = kit.sams_inbox().await;
    let uri = format!("/api/v1/w/{workspace}/drafts");
    let draft = kit
        .post(&sam, &uri, Some(json!({"kind": "new", "inbox_id": inbox})))
        .await;
    let one = format!("{uri}/{}", draft.json["id"].as_str().unwrap());
    let first = kit
        .patch(
            &sam,
            &one,
            json!({"version": 1, "to": [{"address": " ana@example.com "}], "body": "Hi Ana",
                "content_token": draft.json["content_token"]}),
        )
        .await;
    assert_eq!(first.status, StatusCode::OK, "{:?}", first.json);
    assert_eq!(first.json["version"], 2);
    assert_eq!(first.json["to"][0]["address"], "ana@example.com");
    assert_eq!(first.json["body"], "Hi Ana");
    let stale = kit
        .patch(&sam, &one, json!({"version": 1, "body": "Hi"}))
        .await;
    assert_eq!(stale.status, StatusCode::CONFLICT);
    assert_eq!(stale.json["code"], "draft_changed");
    let long = kit
        .patch(
            &sam,
            &one,
            json!({"version": 2, "subject": "a\nb", "content_token": first.json["content_token"]}),
        )
        .await;
    assert_eq!(long.status, StatusCode::BAD_REQUEST);
    assert_eq!(long.json["code"], "invalid_subject");
    // Discarding.
    assert_eq!(kit.delete(&sam, &one).await.status, StatusCode::NO_CONTENT);
    assert_eq!(kit.get(&sam, &one).await.status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_new_message_needs_an_inbox(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, _, sam) = kit.sams_inbox().await;
    let uri = format!("/api/v1/w/{workspace}/drafts");
    let response = kit.post(&sam, &uri, Some(json!({"kind": "new"}))).await;
    assert_eq!(response.status, StatusCode::BAD_REQUEST);
    assert_eq!(response.json["code"], "draft_needs_inbox");
    let response = kit.post(&sam, &uri, Some(json!({"kind": "reply"}))).await;
    assert_eq!(response.json["code"], "draft_needs_message");
}

/// campbooks-cloud#293: a nudge is a reply all of the waiting need's
/// message, to the person waited on, everyone else copied, the inbox in
/// neither; only while the conversation waits on someone.
#[sqlx::test(migrations = "../../../migrations")]
async fn a_nudge_goes_to_the_person_waited_on_and_copies_the_rest(db: PgPool) {
    use crate::testkit::{ANA, JOAO, SAM, joao_asks_ana, waiting_deps};

    let kit = Kit::with_deps(db, waiting_deps()).await;
    let (workspace, inbox, sam) = joao_asks_ana(&kit).await;
    let uri = format!("/api/v1/w/{workspace}/drafts");
    let message = kit.message_of("joao-1").await;
    let created = kit
        .post(
            &sam,
            &uri,
            Some(json!({"kind": "nudge", "message_id": message})),
        )
        .await;
    assert_eq!(created.status, StatusCode::CREATED, "{:?}", created.json);
    let draft = &created.json;
    assert_eq!(draft["kind"], "nudge");
    assert_eq!(addresses(&draft["to"]), [ANA.1]);
    assert_eq!(draft["to"][0]["name"], ANA.0);
    assert_eq!(addresses(&draft["cc"]), [JOAO.1]);
    // The inbox's own address is in neither list.
    for list in ["to", "cc"] {
        assert!(!addresses(&draft[list]).contains(&SAM.1), "{list}");
    }
    assert_eq!(draft["from"]["address"], SAM.1);
    assert_eq!(draft["subject"], "Re: Floor plan");
    assert_eq!(
        draft["conversation_id"],
        kit.conversation_of("joao-1").await
    );
    let (in_reply_to, thread_key): (Option<String>, Option<String>) =
        sqlx::query_as("SELECT in_reply_to, thread_key FROM conversations.drafts")
            .fetch_one(&kit.db)
            .await
            .unwrap();
    assert_eq!(in_reply_to.as_deref(), Some("joao-1@studio.example"));
    assert_eq!(thread_key.as_deref(), Some("thr-floor-plan"));

    // A reader of the inbox may not start one.
    let (reader_token, reader) = kit.member("reader", workspace);
    kit.add_to_inbox(inbox, reader, InboxRole::Reader).await;
    let refused = kit
        .post(
            &reader_token,
            &uri,
            Some(json!({"kind": "nudge", "message_id": message})),
        )
        .await;
    assert_eq!(refused.status, StatusCode::FORBIDDEN, "{:?}", refused.json);

    // Heard from them: nothing to wait for, no nudge, no draft.
    let id = kit.conversation_of("joao-1").await;
    let done = kit
        .post(
            &sam,
            &format!("/api/v1/w/{workspace}/conversations/{id}/need/resolve"),
            Some(json!({"outcome": "done", "message_id": message})),
        )
        .await;
    assert_eq!(done.status, StatusCode::OK, "{:?}", done.json);
    let conflict = kit
        .post(
            &sam,
            &uri,
            Some(json!({"kind": "nudge", "message_id": message})),
        )
        .await;
    assert_eq!(conflict.status, StatusCode::CONFLICT, "{:?}", conflict.json);
    assert_eq!(conflict.json["code"], "not_waiting");
    let drafts: i64 = sqlx::query_scalar("SELECT count(*) FROM conversations.drafts")
        .fetch_one(&kit.db)
        .await
        .unwrap();
    assert_eq!(drafts, 1);
}
