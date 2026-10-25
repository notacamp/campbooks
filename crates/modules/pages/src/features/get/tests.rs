//! Reading a page.

use axum::http::StatusCode;
use campbooks_kernel::tenancy::WorkspaceId;
use serde_json::json;
use sqlx::PgPool;

use crate::testkit::{Kit, docs};

#[sqlx::test(migrations = "../../../migrations")]
async fn a_teammates_private_page_is_not_there_but_a_workspace_page_is(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, _) = kit.member("ana", workspace);
    let (rui, _) = kit.member("rui", workspace);
    let private = kit.add(&ana, workspace, json!({"title": "Notes"})).await;
    let shared = kit
        .add(
            &ana,
            workspace,
            json!({"title": "Handbook", "place": "workspace"}),
        )
        .await;
    let uri = Kit::pages(workspace);

    let hidden = kit
        .get(&rui, &format!("{uri}/{}", private["id"].as_str().unwrap()))
        .await;
    assert_eq!(hidden.status, StatusCode::NOT_FOUND);
    let seen = kit
        .get(&rui, &format!("{uri}/{}", shared["id"].as_str().unwrap()))
        .await;
    assert_eq!(seen.status, StatusCode::OK, "{:?}", seen.json);
    assert_eq!(seen.json["role"], "writer");
    assert_eq!(seen.json["title"], "Handbook");
    // A page that does not exist is the same 404.
    let missing = kit
        .get(&rui, &format!("{uri}/0192f0c4-7d4e-7b39-a7f1-3a8f0e2b9d99"))
        .await;
    assert_eq!(missing.status, StatusCode::NOT_FOUND);
    assert_eq!(missing.json["code"], hidden.json["code"]);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_page_carries_its_body_and_the_pages_above_it(db: PgPool) {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, _) = kit.member("ana", workspace);
    let top = kit.add(&ana, workspace, json!({"title": "Clients"})).await;
    let client = docs::client(1);
    let body = docs::paragraph(&client, "Acme Studio signed on Monday");
    let page = kit
        .add(
            &ana,
            workspace,
            json!({
                "title": "Acme Studio", "parent": top["id"],
                "document": STANDARD.encode(&body),
            }),
        )
        .await;
    let got = kit
        .get(
            &ana,
            &format!("{}/{}", Kit::pages(workspace), page["id"].as_str().unwrap()),
        )
        .await;
    assert_eq!(got.json["ancestors"][0]["title"], "Clients");
    // The document opens to the paragraph.
    let mirror = docs::client(2);
    docs::apply(
        &mirror,
        &STANDARD
            .decode(got.json["document"]["update"].as_str().unwrap())
            .unwrap(),
    );
    assert!(docs::text(&mirror).contains("Acme Studio signed on Monday"));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_ancestor_the_caller_cannot_open_is_left_out_of_the_trail(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, _) = kit.member("ana", workspace);
    let (rui, rui_id) = kit.member("rui", workspace);
    let top = kit
        .add(&ana, workspace, json!({"title": "Secret plans"}))
        .await;
    let leaf = kit
        .add(
            &ana,
            workspace,
            json!({"title": "Weekly plan", "parent": top["id"]}),
        )
        .await;
    let uri = Kit::pages(workspace);
    let leaf_id = leaf["id"].as_str().unwrap();
    // Shared at the leaf only.
    let shared = kit
        .put(
            &ana,
            &format!("{uri}/{leaf_id}/grants/{rui_id}"),
            json!({"role": "reader"}),
        )
        .await;
    assert_eq!(shared.status, StatusCode::OK);
    let got = kit.get(&rui, &format!("{uri}/{leaf_id}")).await;
    assert_eq!(got.status, StatusCode::OK);
    assert_eq!(got.json["ancestors"], json!([]));
    assert_eq!(got.json["role"], "reader");
    assert!(!got.json.to_string().contains("Secret plans"));
    // Its parent is still not there for him.
    let parent = kit
        .get(&rui, &format!("{uri}/{}", top["id"].as_str().unwrap()))
        .await;
    assert_eq!(parent.status, StatusCode::NOT_FOUND);
}
