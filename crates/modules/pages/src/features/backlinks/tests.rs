//! Where a record is linked from.

use axum::http::StatusCode;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use campbooks_kernel::tenancy::WorkspaceId;
use serde_json::json;
use sqlx::PgPool;
use yrs::{Xml, XmlElementPrelim, XmlFragment, XmlTextPrelim};

use crate::testkit::{
    Kit,
    docs::{self, edit, s},
};

/// A body with a paragraph that mentions `ref_type` `ref_id`.
fn mentioning(client: u64, ref_type: &str, ref_id: &str) -> String {
    let update = edit(&docs::client(client), |txn, fragment| {
        let line = fragment.push_back(txn, XmlElementPrelim::empty("paragraph"));
        line.push_back(txn, XmlTextPrelim::new("See "));
        let mention = line.push_back(txn, XmlElementPrelim::empty("mention"));
        mention.insert_attribute(txn, "ref_type", s(ref_type));
        mention.insert_attribute(txn, "ref_id", s(ref_id));
    });
    STANDARD.encode(update)
}

fn backlinks(workspace: WorkspaceId, record_type: &str, id: &str) -> String {
    format!(
        "{}/backlinks?type={record_type}&id={id}",
        Kit::pages(workspace)
    )
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_task_is_linked_from_the_pages_that_mention_it_the_caller_may_open(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, ana_id) = kit.member("ana", workspace);
    let (rui, _) = kit.member("rui", workspace);
    let task = kit.task_of(ana_id);
    kit.add(
        &ana,
        workspace,
        json!({
            "title": "Client onboarding", "place": "workspace",
            "document": mentioning(1, "tasks.task", &task),
        }),
    )
    .await;
    kit.add(
        &ana,
        workspace,
        json!({"title": "My scratch", "document": mentioning(2, "tasks.task", &task)}),
    )
    .await;
    kit.add(&ana, workspace, json!({"title": "Unrelated"}))
        .await;

    let mine = kit
        .get(&ana, &backlinks(workspace, "tasks.task", &task))
        .await;
    assert_eq!(mine.status, StatusCode::OK, "{:?}", mine.json);
    let mut titles: Vec<&str> = mine.json["pages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|page| page["title"].as_str().unwrap())
        .collect();
    titles.sort_unstable();
    assert_eq!(titles, ["Client onboarding", "My scratch"]);

    // Rui cannot read Ana's task: it is not there for him, whatever pages he
    // can open. A mention grants nothing.
    let theirs = kit
        .get(&rui, &backlinks(workspace, "tasks.task", &task))
        .await;
    assert_eq!(theirs.status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_person_is_found_by_address_and_only_on_readable_pages(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, _) = kit.member("ana", workspace);
    let (rui, _) = kit.member("rui", workspace);
    let address = "jonas.weber@example.com";
    kit.add(
        &ana,
        workspace,
        json!({
            "title": "Acme Studio", "place": "workspace",
            "document": mentioning(3, "person", address),
        }),
    )
    .await;
    kit.add(
        &ana,
        workspace,
        json!({"title": "Private notes", "document": mentioning(4, "person", address)}),
    )
    .await;
    let uri = backlinks(workspace, "person", address);
    let for_rui = kit.get(&rui, &uri).await;
    assert_eq!(for_rui.json["pages"].as_array().unwrap().len(), 1);
    assert_eq!(for_rui.json["pages"][0]["title"], "Acme Studio");
    let for_ana = kit.get(&ana, &uri).await;
    assert_eq!(for_ana.json["pages"].as_array().unwrap().len(), 2);
    // An address that is not one, and a kind of record a page cannot link to.
    assert_eq!(
        kit.get(&ana, &backlinks(workspace, "person", "Jonas%20Weber"))
            .await
            .status,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        kit.get(&ana, &backlinks(workspace, "documents.document", "x"))
            .await
            .status,
        StatusCode::BAD_REQUEST
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_page_in_the_trash_and_the_page_itself_are_not_backlinks(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, _) = kit.member("ana", workspace);
    let target = kit.add(&ana, workspace, json!({"title": "Target"})).await;
    let target_id = target["id"].as_str().unwrap();
    let linking = kit
        .add(
            &ana,
            workspace,
            json!({"title": "Links to it", "document": mentioning(5, "pages.page", target_id)}),
        )
        .await;
    let uri = backlinks(workspace, "pages.page", target_id);
    let found = kit.get(&ana, &uri).await;
    assert_eq!(found.json["pages"].as_array().unwrap().len(), 1);
    kit.post(
        &ana,
        &format!(
            "{}/{}/trash",
            Kit::pages(workspace),
            linking["id"].as_str().unwrap()
        ),
        None,
    )
    .await;
    let found = kit.get(&ana, &uri).await;
    assert_eq!(found.json["pages"], json!([]));
}
