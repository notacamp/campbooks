//! What a member's edits ask of Notion: quiet first, one request at a time,
//! bound so the connector can check it, and with no words in any event.

use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;
use yrs::{Xml, XmlElementPrelim, XmlFragment, XmlTextPrelim};

use crate::{
    domain::notion::tests::{id, para, snapshot},
    testkit::docs,
    testkit_notion::{Rig, assert_contract},
};

/// A page linked to a Notion page and read once: "Plan the launch" and
/// "Write the notes".
async fn synced(db: PgPool) -> (Rig, String, Uuid) {
    let rig = Rig::new(db).await;
    let notion_page = id(0xAA);
    let page = rig.import(notion_page, "Roadmap").await;
    let mut snap = snapshot(
        vec![para(1, "Plan the launch", 1), para(2, "Write the notes", 1)],
        5,
        true,
    );
    snap.notion_page = notion_page;
    rig.read(&page, notion_page, &snap).await;
    (rig, page, notion_page)
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_edit_becomes_one_bound_request_and_no_event_carries_a_word_of_it(db: PgPool) {
    let (rig, page, notion_page) = synced(db).await;
    rig.type_into(&page, 1, " by Friday").await;
    rig.push_now().await;
    let requests = rig.requests(&page).await;
    assert_eq!(requests.len(), 1);
    let (request, state, items) = &requests[0];
    assert_eq!(state, "waiting");
    let items = items.as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["action"], "update");
    assert_eq!(items[0]["external"], id(2).to_string());
    // The request is stored with ids, kinds, times and a hash: no text.
    assert!(!requests[0].2.to_string().contains("Friday"));

    let events = rig.kit.events("pages.notion_changes_requested").await;
    assert_eq!(events.len(), 1);
    assert_contract("pages.notion_changes_requested.v1.schema.json", &events[0]);
    assert_eq!(events[0]["request"], request.to_string());
    assert_eq!(events[0]["notion_page"], notion_page.to_string());
    assert!(!events[0].to_string().contains("Friday"));

    // The words are in the document the request wrote, a blob.
    let document = rig.blob_json(&events[0]["body"]).await;
    assert_contract("notion-block-changes.v1.schema.json", &document);
    assert_eq!(
        document["items"][0]["block"]["runs"][0]["text"],
        "Write the notes by Friday"
    );

    let stood = rig.stands(*request).await.expect("the request stands");
    assert_eq!(stood.external_page, notion_page);
    assert_eq!(stood.connection, rig.connection);
    assert_eq!(stood.items.len(), 1);
    assert_eq!(stood.items[0].external, Some(id(2)));
    assert!(stood.items[0].base_edited_at.is_some());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn nothing_is_sent_before_the_page_is_quiet_or_when_nothing_changed(db: PgPool) {
    let (rig, page, _) = synced(db).await;
    // Nothing changed: nothing asked.
    rig.push_now().await;
    assert!(rig.requests(&page).await.is_empty());
    // Just typed: the job waits for the page to be quiet.
    rig.type_into(&page, 0, " now").await;
    rig.kit.run_jobs().await;
    assert!(rig.requests(&page).await.is_empty());
    let waiting: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM kernel.jobs WHERE kind = 'pages.push_to_notion' AND state = 'pending'",
    )
    .fetch_one(&rig.kit.db)
    .await
    .unwrap();
    assert_eq!(waiting, 1);
    rig.push_now().await;
    assert_eq!(rig.requests(&page).await.len(), 1);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn one_request_waits_at_a_time(db: PgPool) {
    let (rig, page, _) = synced(db).await;
    rig.type_into(&page, 0, " one").await;
    rig.push_now().await;
    rig.type_into(&page, 1, " two").await;
    rig.push_now().await;
    assert_eq!(
        rig.requests(&page).await.len(),
        1,
        "the second waits for the first"
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_new_paragraph_is_an_append_after_the_block_before_it(db: PgPool) {
    let (rig, page, _) = synced(db).await;
    rig.edit_client(&page, |txn, fragment| {
        let line = fragment.push_back(txn, XmlElementPrelim::empty("paragraph"));
        line.push_back(txn, XmlTextPrelim::new("A new line"));
    })
    .await;
    rig.push_now().await;
    let requests = rig.requests(&page).await;
    let items = requests[0].2.as_array().unwrap();
    assert_eq!(items[0]["action"], "append");
    assert_eq!(items[0]["after"], id(2).to_string());
    assert_eq!(items[0]["external"], Value::Null);
    // It has a stable id now.
    assert!(rig.tree(&page).await.nodes.iter().all(|n| n.bid.is_some()));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_mention_of_anything_but_a_synced_page_goes_as_campbooks_link_with_no_subject(
    db: PgPool,
) {
    let (rig, page, _) = synced(db).await;
    let task = rig.kit.task_of(rig.user);
    let person = "marta.example@example.com";
    rig.edit_client(&page, |txn, fragment| {
        let line = fragment.push_back(txn, XmlElementPrelim::empty("paragraph"));
        line.push_back(txn, XmlTextPrelim::new("ask "));
        for (ref_type, ref_id) in [("tasks.task", task.as_str()), ("person", person)] {
            let mention = line.push_back(txn, XmlElementPrelim::empty("mention"));
            mention.insert_attribute(txn, "ref_type", docs::s(ref_type));
            mention.insert_attribute(txn, "ref_id", docs::s(ref_id));
        }
    })
    .await;
    rig.push_now().await;
    let events = rig.kit.events("pages.notion_changes_requested").await;
    let document = rig.blob_json(&events[0]["body"]).await;
    let runs = document["items"][0]["block"]["runs"].as_array().unwrap();
    let text: String = runs.iter().map(|r| r["text"].as_str().unwrap()).collect();
    // Whatever was mentioned, only the words "Campbooks link" stand for it.
    assert_eq!(text.replace("Campbooks link", ""), "ask ", "{runs:?}");
    assert!(text.contains("Campbooks link"));
    let shown = document.to_string();
    // The task's address is this app's; the person is never named.
    assert!(!shown.contains("marta.example"), "{shown}");
    assert!(!shown.contains('@'), "{shown}");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_request_the_connector_may_not_trust_does_not_stand(db: PgPool) {
    let (rig, page, _) = synced(db).await;
    rig.type_into(&page, 0, " edit").await;
    rig.push_now().await;
    let request = rig.requests(&page).await[0].0;
    assert!(rig.stands(request).await.is_some());

    // Another request id, a request of nobody's.
    assert!(rig.stands(Uuid::now_v7()).await.is_none());

    // Edited after it was bound: it points at another block.
    sqlx::query(
        "UPDATE pages.notion_requests
            SET items = jsonb_set(items, '{0,external}', to_jsonb($2::text))
          WHERE id = $1",
    )
    .bind(request)
    .bind(id(2).to_string())
    .execute(&rig.kit.db)
    .await
    .unwrap();
    assert!(
        rig.stands(request).await.is_none(),
        "a tampered item is refused"
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_request_without_its_binding_does_not_stand(db: PgPool) {
    let (rig, page, _) = synced(db).await;
    rig.type_into(&page, 0, " edit").await;
    rig.push_now().await;
    let request = rig.requests(&page).await[0].0;
    sqlx::query("UPDATE pages.notion_requests SET mac = NULL")
        .execute(&rig.kit.db)
        .await
        .unwrap();
    assert!(rig.stands(request).await.is_none());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_request_does_not_stand_once_the_link_ended_or_in_another_workspace(db: PgPool) {
    let (rig, page, _) = synced(db).await;
    rig.type_into(&page, 0, " edit").await;
    rig.push_now().await;
    let request = rig.requests(&page).await[0].0;
    // Another workspace asks about it: not found there (and not bound there).
    let elsewhere = campbooks_kernel::tenancy::WorkspaceId::new();
    {
        let mut tx = rig.kit.begin(elsewhere).await;
        let crossed = crate::domain::notion::requests::page_requests()
            .stored_changes(&rig.kit.state, &mut tx, elsewhere, request)
            .await
            .unwrap();
        assert!(crossed.is_none());
    }
    let stop = format!("{}/{page}/notion/stop", crate::testkit::Kit::pages(rig.ws));
    assert_eq!(
        rig.kit.post(&rig.token, &stop, None).await.status.as_u16(),
        200
    );
    assert!(rig.stands(request).await.is_none());
    // The stopped link ended its waiting request too.
    assert_eq!(rig.requests(&page).await[0].1, "done");
}
