//! Listing the pages a member may open.

use campbooks_kernel::tenancy::WorkspaceId;
use serde_json::json;
use sqlx::PgPool;

use crate::testkit::Kit;

fn titles(list: &serde_json::Value) -> Vec<String> {
    list["pages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|page| page["title"].as_str().unwrap().to_owned())
        .collect()
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_list_has_only_what_the_caller_may_open(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, _) = kit.member("ana", workspace);
    let (rui, rui_id) = kit.member("rui", workspace);
    let (wes, _) = kit.owner("wes", workspace);
    kit.add(&ana, workspace, json!({"title": "Ana private"}))
        .await;
    let shared = kit
        .add(&ana, workspace, json!({"title": "Ana shared with Rui"}))
        .await;
    kit.add(
        &ana,
        workspace,
        json!({"title": "Handbook", "place": "workspace"}),
    )
    .await;
    kit.add(&rui, workspace, json!({"title": "Rui private"}))
        .await;
    let uri = Kit::pages(workspace);
    kit.put(
        &ana,
        &format!("{uri}/{}/grants/{rui_id}", shared["id"].as_str().unwrap()),
        json!({"role": "reader"}),
    )
    .await;

    let mut for_rui = titles(&kit.get(&rui, &uri).await.json);
    for_rui.sort();
    assert_eq!(for_rui, ["Ana shared with Rui", "Handbook", "Rui private"]);
    let mut for_ana = titles(&kit.get(&ana, &uri).await.json);
    for_ana.sort();
    assert_eq!(for_ana, ["Ana private", "Ana shared with Rui", "Handbook"]);
    // A workspace owner sees the workspace's pages and not the private ones.
    assert_eq!(titles(&kit.get(&wes, &uri).await.json), ["Handbook"]);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_readable_sub_page_under_an_unreadable_parent_is_listed_with_its_role(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, _) = kit.member("ana", workspace);
    let (rui, rui_id) = kit.member("rui", workspace);
    let parent = kit.add(&ana, workspace, json!({"title": "Parent"})).await;
    let child = kit
        .add(
            &ana,
            workspace,
            json!({"title": "Child", "parent": parent["id"]}),
        )
        .await;
    let uri = Kit::pages(workspace);
    kit.put(
        &ana,
        &format!("{uri}/{}/grants/{rui_id}", child["id"].as_str().unwrap()),
        json!({"role": "writer"}),
    )
    .await;
    let list = kit.get(&rui, &uri).await.json;
    assert_eq!(titles(&list), ["Child"]);
    assert_eq!(list["pages"][0]["role"], "writer");
    assert_eq!(list["pages"][0]["parent"], parent["id"]);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn siblings_come_in_order_and_the_trash_lists_what_was_deleted(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, _) = kit.member("ana", workspace);
    let uri = Kit::pages(workspace);
    let first = kit.add(&ana, workspace, json!({"title": "First"})).await;
    kit.add(&ana, workspace, json!({"title": "Second"})).await;
    let third = kit.add(&ana, workspace, json!({"title": "Third"})).await;
    assert_eq!(
        titles(&kit.get(&ana, &uri).await.json),
        ["First", "Second", "Third"]
    );
    let under = kit
        .add(
            &ana,
            workspace,
            json!({"title": "Inside", "parent": third["id"]}),
        )
        .await;
    assert_eq!(under["parent"], third["id"]);
    let trashed = kit
        .post(
            &ana,
            &format!("{uri}/{}/trash", third["id"].as_str().unwrap()),
            None,
        )
        .await;
    assert_eq!(
        trashed.status,
        axum::http::StatusCode::OK,
        "{:?}",
        trashed.json
    );
    assert_eq!(titles(&kit.get(&ana, &uri).await.json), ["First", "Second"]);
    // Only the page that was deleted is in the trash list, not what went
    // with it.
    assert_eq!(
        titles(&kit.get(&ana, &format!("{uri}?trashed=true")).await.json),
        ["Third"]
    );
    let children = kit
        .get(
            &ana,
            &format!("{uri}?parent={}", first["id"].as_str().unwrap()),
        )
        .await;
    assert_eq!(titles(&children.json), Vec::<String>::new());
}
