//! Taking back files saved from a message.

use axum::http::StatusCode;
use campbooks_kernel::tenancy::WorkspaceId;
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;
use yrs::Any;

use crate::testkit::{Kit, docs};

struct Fixture {
    kit: Kit,
    workspace: WorkspaceId,
    ana: String,
    page: String,
    agenda: String,
    budget: String,
}

/// Ana's page, holding two files saved from a message.
async fn fixture(db: PgPool) -> Fixture {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, ana_id) = kit.member("ana", workspace);
    let page = kit
        .add(&ana, workspace, json!({"title": "Client onboarding"}))
        .await;
    let page = page["id"].as_str().unwrap().to_owned();
    let message = (Uuid::now_v7(), Uuid::now_v7());
    let agenda = kit
        .mail_attachment(workspace, ana_id, message, "Agenda.pdf", b"%PDF agenda")
        .await;
    let budget = kit
        .mail_attachment(workspace, ana_id, message, "Budget.pdf", b"%PDF budget")
        .await;
    let attached = kit
        .post(
            &ana,
            &format!("{}/{page}/files/from-message", Kit::pages(workspace)),
            Some(json!({"conversation": message.0, "message": message.1, "attachments": [agenda, budget]})),
        )
        .await;
    assert_eq!(attached.status, StatusCode::OK, "{:?}", attached.json);
    let blob = |n: usize| {
        attached.json["files"][n]["blob"]
            .as_str()
            .unwrap()
            .to_owned()
    };
    Fixture {
        agenda: blob(0),
        budget: blob(1),
        kit,
        workspace,
        ana,
        page,
    }
}

impl Fixture {
    async fn detach(&self, token: &str, files: &[&str]) -> campbooks_kernel::testing::TestResponse {
        self.kit
            .post(
                token,
                &format!("{}/{}/files/detach", Kit::pages(self.workspace), self.page),
                Some(json!({"files": files})),
            )
            .await
    }

    async fn files(&self) -> Vec<String> {
        sqlx::query_scalar("SELECT hash FROM pages.files ORDER BY hash")
            .fetch_all(&self.kit.db)
            .await
            .unwrap()
    }

    async fn holds(&self) -> i64 {
        sqlx::query_scalar("SELECT count(*) FROM kernel.blob_refs WHERE owner_type = 'pages.page'")
            .fetch_one(&self.kit.db)
            .await
            .unwrap()
    }
}

#[sqlx::test(migrations = "../../../migrations")]
async fn files_no_block_names_are_let_go_and_the_page_holds_nothing_of_them(db: PgPool) {
    let f = fixture(db).await;
    let undone = f.detach(&f.ana, &[&f.agenda, &f.budget]).await;
    assert_eq!(undone.status, StatusCode::OK, "{:?}", undone.json);
    assert_eq!(undone.json["removed"].as_array().unwrap().len(), 2);
    assert_eq!(undone.json["kept"], json!([]));
    assert!(f.files().await.is_empty());
    assert_eq!(f.holds().await, 0);
    // Again, there is nothing to let go of: it says so.
    let again = f.detach(&f.ana, &[&f.agenda]).await;
    assert_eq!(again.json["removed"], json!([]));
    assert_eq!(again.json["kept"], json!([f.agenda]));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_file_a_block_names_stays(db: PgPool) {
    let f = fixture(db).await;
    let client = docs::client(71);
    let update = docs::element(
        &client,
        "file",
        &[
            ("blob", docs::s(&f.agenda)),
            ("name", docs::s("Agenda.pdf")),
            ("size", Any::Number(11.0)),
        ],
        None,
    );
    let synced = f
        .kit
        .sync(&f.ana, f.workspace, &f.page, &client, Some(&update))
        .await;
    assert_eq!(synced.status, StatusCode::OK, "{:?}", synced.json);

    let undone = f.detach(&f.ana, &[&f.agenda, &f.budget]).await;
    assert_eq!(undone.json["removed"], json!([f.budget]));
    assert_eq!(undone.json["kept"], json!([f.agenda]));
    assert_eq!(f.files().await, vec![f.agenda.clone()]);
    assert_eq!(f.holds().await, 1);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_file_the_member_uploaded_is_never_taken_back(db: PgPool) {
    let f = fixture(db).await;
    let uploaded = f
        .kit
        .upload(
            &f.ana,
            &format!("{}/{}/files", Kit::pages(f.workspace), f.page),
            "application/pdf",
            b"%PDF mine".to_vec(),
        )
        .await;
    assert_eq!(uploaded.status, StatusCode::CREATED, "{:?}", uploaded.json);
    let mine = uploaded.json["blob"].as_str().unwrap().to_owned();
    let undone = f.detach(&f.ana, &[&mine]).await;
    assert_eq!(undone.json["removed"], json!([]));
    assert_eq!(undone.json["kept"], json!([mine]));
    assert!(f.files().await.contains(&mine));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn who_may_ask_and_what_may_be_asked(db: PgPool) {
    let f = fixture(db).await;
    let (rui, rui_id) = f.kit.member("rui", f.workspace);
    assert_eq!(
        f.detach(&rui, &[&f.agenda]).await.status,
        StatusCode::NOT_FOUND
    );
    f.kit
        .share(&f.ana, f.workspace, &f.page, rui_id, "reader")
        .await;
    assert_eq!(
        f.detach(&rui, &[&f.agenda]).await.status,
        StatusCode::FORBIDDEN
    );
    assert_eq!(f.files().await.len(), 2);

    for bad in [
        json!([]),
        json!(["not a hash"]),
        json!(
            vec!["a".repeat(64); 1]
                .into_iter()
                .chain((0..20).map(|n| format!("{n:064x}")))
                .collect::<Vec<_>>()
        ),
    ] {
        let answer: Value = f
            .kit
            .post(
                &f.ana,
                &format!("{}/{}/files/detach", Kit::pages(f.workspace), f.page),
                Some(json!({"files": bad})),
            )
            .await
            .json;
        assert_eq!(answer["code"], "invalid_attachments", "{answer}");
    }
    assert_eq!(f.files().await.len(), 2);
}
