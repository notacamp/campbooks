//! Who reads a page, as Scout's cards say it.

use axum::http::StatusCode;
use campbooks_kernel::tenancy::WorkspaceId;
use serde_json::{Value, json};
use sqlx::PgPool;

use crate::testkit::Kit;

async fn sharing(kit: &Kit, token: &str, workspace: WorkspaceId, page: &str) -> Value {
    let got = kit
        .get(token, &format!("{}/{page}/sharing", Kit::pages(workspace)))
        .await;
    assert_eq!(got.status, StatusCode::OK, "{:?}", got.json);
    got.json
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_private_page_is_its_creators_alone_until_it_is_shared(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, ana_id) = kit.member("ana", workspace);
    let (_, rui_id) = kit.member("rui", workspace);
    let page = kit.add(&ana, workspace, json!({"title": "Notes"})).await;
    let id = page["id"].as_str().unwrap();
    let got = sharing(&kit, &ana, workspace, id).await;
    assert_eq!(got["place"], "private");
    assert_eq!(got["everyone"], false);
    assert_eq!(got["readers"], json!([ana_id]));
    assert_eq!(got["notion"], false);

    kit.share(&ana, workspace, id, rui_id, "reader").await;
    let got = sharing(&kit, &ana, workspace, id).await;
    let mut readers: Vec<String> = got["readers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r.as_str().unwrap().to_owned())
        .collect();
    readers.sort();
    let mut expected = vec![ana_id.to_string(), rui_id.to_string()];
    expected.sort();
    assert_eq!(readers, expected);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_grant_on_a_page_above_reaches_its_sub_pages(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, ana_id) = kit.member("ana", workspace);
    let (_, rui_id) = kit.member("rui", workspace);
    let top = kit.add(&ana, workspace, json!({"title": "Clients"})).await;
    let top_id = top["id"].as_str().unwrap();
    let sub = kit
        .add(&ana, workspace, json!({"title": "Acme", "parent": top_id}))
        .await;
    kit.share(&ana, workspace, top_id, rui_id, "writer").await;
    // The sub-page has no grant of its own: Rui reads it all the same.
    let got = sharing(&kit, &ana, workspace, sub["id"].as_str().unwrap()).await;
    let readers = got["readers"].as_array().unwrap();
    assert!(readers.contains(&json!(rui_id)) && readers.contains(&json!(ana_id)));
    assert_eq!(readers.len(), 2);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_workspace_page_is_everyones(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, _) = kit.member("ana", workspace);
    let page = kit
        .add(
            &ana,
            workspace,
            json!({"title": "Plan", "place": "workspace"}),
        )
        .await;
    let got = sharing(&kit, &ana, workspace, page["id"].as_str().unwrap()).await;
    assert_eq!(got["place"], "workspace");
    assert_eq!(got["everyone"], true);
    assert_eq!(got["readers"], json!([]));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_page_kept_in_step_with_notion_says_so(db: PgPool) {
    use crate::{
        domain::notion::tests::{id, para, snapshot},
        testkit_notion::Rig,
    };
    let rig = Rig::new(db).await;
    let notion_page = id(0xAA);
    let page = rig.import(notion_page, "Roadmap").await;
    let mut snap = snapshot(vec![para(1, "Plan the launch", 1)], 5, true);
    snap.notion_page = notion_page;
    rig.read(&page, notion_page, &snap).await;
    let got = sharing(&rig.kit, &rig.token, rig.ws, &page).await;
    assert_eq!(got["notion"], true);
    // Stopped, it is not.
    let stopped = rig
        .kit
        .post(
            &rig.token,
            &format!("{}/{page}/notion/stop", Kit::pages(rig.ws)),
            None,
        )
        .await;
    assert!(stopped.status.is_success(), "{:?}", stopped.json);
    assert_eq!(
        sharing(&rig.kit, &rig.token, rig.ws, &page).await["notion"],
        false
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn only_a_reader_of_the_page_may_ask_and_it_is_not_there_while_off(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, _) = kit.member("ana", workspace);
    let (rui, _) = kit.member("rui", workspace);
    let page = kit.add(&ana, workspace, json!({"title": "Notes"})).await;
    let uri = format!(
        "{}/{}/sharing",
        Kit::pages(workspace),
        page["id"].as_str().unwrap()
    );
    assert_eq!(kit.get(&rui, &uri).await.status, StatusCode::NOT_FOUND);
    let off = kit.with_mail_scout(false);
    assert_eq!(off.get(&ana, &uri).await.status, StatusCode::NOT_FOUND);
}
