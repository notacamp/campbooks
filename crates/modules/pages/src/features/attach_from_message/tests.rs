//! Saving a message's attachments to a page.

use axum::http::StatusCode;
use campbooks_kernel::{tenancy::WorkspaceId, testing};
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
    /// The message the files are attached to: Ana alone reads it.
    message: (Uuid, Uuid),
    ana_id: campbooks_kernel::tenancy::UserId,
}

async fn fixture(db: PgPool) -> Fixture {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, ana_id) = kit.member("ana", workspace);
    let page = kit
        .add(&ana, workspace, json!({"title": "Client onboarding"}))
        .await;
    Fixture {
        page: page["id"].as_str().unwrap().to_owned(),
        kit,
        workspace,
        ana,
        message: (Uuid::now_v7(), Uuid::now_v7()),
        ana_id,
    }
}

impl Fixture {
    fn uri(&self, tail: &str) -> String {
        format!("{}/{}/{tail}", Kit::pages(self.workspace), self.page)
    }

    async fn attachment(&self, name: &str, bytes: &[u8]) -> Uuid {
        self.kit
            .mail_attachment(self.workspace, self.ana_id, self.message, name, bytes)
            .await
    }

    async fn attach(&self, token: &str, attachments: &[Uuid]) -> testing::TestResponse {
        self.attach_to(token, self.message, attachments).await
    }

    async fn attach_to(
        &self,
        token: &str,
        (conversation, message): (Uuid, Uuid),
        attachments: &[Uuid],
    ) -> testing::TestResponse {
        self.kit
            .post(
                token,
                &self.uri("files/from-message"),
                Some(json!({
                    "conversation": conversation,
                    "message": message,
                    "attachments": attachments,
                })),
            )
            .await
    }

    async fn files(&self) -> i64 {
        sqlx::query_scalar("SELECT count(*) FROM pages.files")
            .fetch_one(&self.kit.db)
            .await
            .unwrap()
    }

    async fn blobs(&self) -> i64 {
        sqlx::query_scalar("SELECT count(*) FROM kernel.blobs")
            .fetch_one(&self.kit.db)
            .await
            .unwrap()
    }
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_readers_attachments_become_the_pages_files_without_copying_a_byte(db: PgPool) {
    let f = fixture(db).await;
    let agenda = f.attachment("Kickoff agenda.pdf", b"%PDF agenda").await;
    let budget = f.attachment("Budget.pdf", b"%PDF budget").await;
    let stored = f.blobs().await;

    let attached = f.attach(&f.ana, &[agenda, budget]).await;
    assert_eq!(attached.status, StatusCode::OK, "{:?}", attached.json);
    let files = attached.json["files"].as_array().unwrap();
    assert_eq!(files.len(), 2);
    assert_eq!(files[0]["attachment"], agenda.to_string());
    assert_eq!(files[0]["name"], "Kickoff agenda.pdf");
    assert_eq!(files[0]["content_type"], "application/pdf");
    assert_eq!(files[0]["size"], 11);
    assert_eq!(files[1]["name"], "Budget.pdf");

    // The page holds both, with where they came from, and no new blob was
    // stored.
    assert_eq!(f.files().await, 2);
    assert_eq!(f.blobs().await, stored);
    let from: Vec<(String, String)> =
        sqlx::query_as("SELECT source_type, source_id FROM pages.files ORDER BY source_id")
            .fetch_all(&f.kit.db)
            .await
            .unwrap();
    assert!(
        from.iter()
            .all(|(kind, _)| kind == "conversations.attachment")
    );
    let held: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM kernel.blob_refs WHERE owner_type = 'pages.page' AND owner_id = $1",
    )
    .bind(f.page.parse::<Uuid>().unwrap())
    .fetch_one(&f.kit.db)
    .await
    .unwrap();
    assert_eq!(held, 2);

    // The page's own file route serves the bytes to those who read the page.
    let blob = files[0]["blob"].as_str().unwrap();
    let (status, _, bytes) = f
        .kit
        .download(&f.ana, &f.uri(&format!("files/{blob}")))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(bytes, b"%PDF agenda");

    // And the client's file block naming the blob is accepted by the checker.
    let client = docs::client(61);
    let update = docs::element(
        &client,
        "file",
        &[
            ("blob", docs::s(blob)),
            ("name", docs::s("Kickoff agenda.pdf")),
            ("size", Any::Number(11.0)),
        ],
        None,
    );
    let synced = f
        .kit
        .sync(&f.ana, f.workspace, &f.page, &client, Some(&update))
        .await;
    assert_eq!(synced.status, StatusCode::OK, "{:?}", synced.json);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn one_attachment_the_caller_may_not_read_makes_the_whole_call_a_404(db: PgPool) {
    let f = fixture(db).await;
    let (rui, rui_id) = f.kit.member("rui", f.workspace);
    f.kit
        .share(&f.ana, f.workspace, &f.page, rui_id, "writer")
        .await;
    let mine = f
        .kit
        .mail_attachment(f.workspace, rui_id, f.message, "Mine.pdf", b"%PDF mine")
        .await;
    let secret = f.attachment("Secret.pdf", b"%PDF secret").await;
    let stored = f.blobs().await;

    // Rui writes the page but does not read Ana's message: 404, and the
    // attachment he may read is not attached either.
    let refused = f.attach(&rui, &[mine, secret]).await;
    assert_eq!(refused.status, StatusCode::NOT_FOUND, "{:?}", refused.json);
    assert!(!refused.json.to_string().contains("Secret"));
    assert_eq!(f.files().await, 0);
    assert_eq!(f.blobs().await, stored);
    let held: i64 =
        sqlx::query_scalar("SELECT count(*) FROM kernel.blob_refs WHERE owner_type = 'pages.page'")
            .fetch_one(&f.kit.db)
            .await
            .unwrap();
    assert_eq!(held, 0);
    // The answer is the same as for an attachment that does not exist.
    let missing = f.attach(&rui, &[Uuid::now_v7()]).await;
    assert_eq!(missing.status, StatusCode::NOT_FOUND);
    assert_eq!(missing.json, refused.json);

    // The right attachment of a wrong message or conversation: the same.
    let wrong_message = f
        .attach_to(&f.ana, (f.message.0, Uuid::now_v7()), &[secret])
        .await;
    assert_eq!(wrong_message.status, StatusCode::NOT_FOUND);
    let wrong_conversation = f
        .attach_to(&f.ana, (Uuid::now_v7(), f.message.1), &[secret])
        .await;
    assert_eq!(wrong_conversation.status, StatusCode::NOT_FOUND);
    assert_eq!(f.files().await, 0);

    // One good and one missing, from the reader: nothing either.
    let half = f.attach(&f.ana, &[secret, Uuid::now_v7()]).await;
    assert_eq!(half.status, StatusCode::NOT_FOUND);
    assert_eq!(f.files().await, 0);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_page_must_be_writable_and_live_and_a_page(db: PgPool) {
    let f = fixture(db).await;
    let (rui, rui_id) = f.kit.member("rui", f.workspace);
    let attachment = f
        .kit
        .mail_attachment(f.workspace, rui_id, f.message, "Rui's.pdf", b"%PDF")
        .await;
    // Rui cannot see Ana's private page.
    assert_eq!(
        f.attach(&rui, &[attachment]).await.status,
        StatusCode::NOT_FOUND
    );
    // A reader of it cannot change it.
    f.kit
        .share(&f.ana, f.workspace, &f.page, rui_id, "reader")
        .await;
    assert_eq!(
        f.attach(&rui, &[attachment]).await.status,
        StatusCode::FORBIDDEN
    );
    assert_eq!(f.files().await, 0);

    // A table has no body.
    let table = f.kit.table(&f.ana, f.workspace).await["page"]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let own = f.attachment("Own.pdf", b"%PDF own").await;
    let on_table = f
        .kit
        .post(
            &f.ana,
            &format!("{}/{}/files/from-message", Kit::pages(f.workspace), table),
            Some(
                json!({"conversation": f.message.0, "message": f.message.1, "attachments": [own]}),
            ),
        )
        .await;
    assert_eq!(on_table.status, StatusCode::CONFLICT);

    // The trash is not a place to put files.
    f.kit.post(&f.ana, &f.uri("trash"), None).await;
    let trashed = f.attach(&f.ana, &[own]).await;
    assert_eq!(trashed.status, StatusCode::CONFLICT);
    assert_eq!(trashed.json["code"], "page_trashed");
    assert_eq!(f.files().await, 0);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn between_one_and_twenty_attachments_each_once_and_none_over_twenty_mebibytes(db: PgPool) {
    let f = fixture(db).await;
    let one = f.attachment("One.pdf", b"%PDF one").await;
    assert_eq!(f.attach(&f.ana, &[]).await.status, StatusCode::BAD_REQUEST);
    let many: Vec<Uuid> = (0..21).map(|_| Uuid::now_v7()).collect();
    let too_many = f.attach(&f.ana, &many).await;
    assert_eq!(too_many.status, StatusCode::BAD_REQUEST);
    assert_eq!(too_many.json["code"], "invalid_attachments");
    // The same attachment twice is one file.
    let twice = f.attach(&f.ana, &[one, one]).await;
    assert_eq!(twice.status, StatusCode::OK, "{:?}", twice.json);
    assert_eq!(twice.json["files"].as_array().unwrap().len(), 1);
    assert_eq!(f.files().await, 1);
    // And again is still one.
    f.attach(&f.ana, &[one]).await;
    assert_eq!(f.files().await, 1);

    let huge = f
        .attachment("Huge.bin", &vec![7u8; 20 * 1024 * 1024 + 1])
        .await;
    let refused = f.attach(&f.ana, &[huge]).await;
    assert_eq!(
        refused.status,
        StatusCode::PAYLOAD_TOO_LARGE,
        "{:?}",
        refused.json
    );
    assert_eq!(refused.json["code"], "file_too_large");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_file_name_is_cleaned_for_a_block(db: PgPool) {
    let f = fixture(db).await;
    let odd = f
        .attachment("invoice\u{202e}fdp.exe\u{200b}\n\t.pdf", b"%PDF")
        .await;
    let long = f.attachment(&"n".repeat(400), b"%PDF 2").await;
    let blank = f.attachment("\u{202e}\u{200b}", b"%PDF 3").await;
    let attached = f.attach(&f.ana, &[odd, long, blank]).await;
    assert_eq!(attached.status, StatusCode::OK, "{:?}", attached.json);
    let names: Vec<&str> = attached.json["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|file| file["name"].as_str().unwrap())
        .collect();
    assert_eq!(names[0], "invoicefdp.exe.pdf");
    assert_eq!(names[1].chars().count(), 255);
    assert_eq!(names[2], "file");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn saving_files_is_tracked_as_a_count_and_nothing_else(db: PgPool) {
    let kit = Kit::new(db).await.tracking();
    let workspace = WorkspaceId::new();
    let (ana, ana_id) = kit.member("ana", workspace);
    let page = kit
        .add(&ana, workspace, json!({"title": "Secret plan"}))
        .await;
    let message = (Uuid::now_v7(), Uuid::now_v7());
    let a = kit
        .mail_attachment(workspace, ana_id, message, "Secret.pdf", b"%PDF a")
        .await;
    let b = kit
        .mail_attachment(workspace, ana_id, message, "Other.pdf", b"%PDF b")
        .await;
    let attached = kit
        .post(
            &ana,
            &format!(
                "{}/{}/files/from-message",
                Kit::pages(workspace),
                page["id"].as_str().unwrap()
            ),
            Some(json!({"conversation": message.0, "message": message.1, "attachments": [a, b]})),
        )
        .await;
    assert_eq!(attached.status, StatusCode::OK, "{:?}", attached.json);
    // A refused call tracks nothing.
    kit.post(
        &ana,
        &format!("{}/{}/files/from-message", Kit::pages(workspace), page["id"].as_str().unwrap()),
        Some(json!({"conversation": message.0, "message": message.1, "attachments": [Uuid::now_v7()]})),
    )
    .await;

    let tracked = testing::tracked_events(&kit.db, "pages.files_attached").await;
    assert_eq!(tracked.len(), 1);
    assert_eq!(
        tracked[0].envelope["operation"],
        "pages.attach_from_message"
    );
    assert_eq!(tracked[0].body["files"], 2);
    assert_eq!(tracked[0].body["page"]["id"], page["id"]);
    let body: Value = tracked[0].body.clone();
    let text = body.to_string();
    for word in [
        "Secret",
        "Other",
        "pdf",
        &message.0.to_string(),
        &message.1.to_string(),
    ] {
        assert!(!text.contains(word), "{text}");
    }
}

#[sqlx::test(migrations = "../../../migrations")]
async fn without_the_mail_modules_port_nothing_is_ever_attached(db: PgPool) {
    let f = fixture(db).await;
    let attachment = f.attachment("Kickoff agenda.pdf", b"%PDF agenda").await;
    let f = Fixture {
        kit: f.kit.without_mail(),
        ..f
    };
    let refused = f.attach(&f.ana, &[attachment]).await;
    assert_eq!(refused.status, StatusCode::NOT_FOUND, "{:?}", refused.json);
    assert_eq!(f.files().await, 0);
}
