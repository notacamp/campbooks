//! Adding a section at the end of a page.

use axum::http::StatusCode;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use campbooks_kernel::tenancy::WorkspaceId;
use serde_json::{Value, json};
use sqlx::PgPool;

use crate::{
    domain::markdown::MAX_MARKDOWN_BYTES,
    testkit::{Kit, docs},
    testkit_notion::Rig,
};

const THREAD: &str = "0192f0c4-7d4e-7b39-a7f1-3a8f0e2b9c22";
const TASK: &str = "0192f0c4-7d4e-7b39-a7f1-3a8f0e2b9c33";

fn page_uri(workspace: WorkspaceId, page: &str) -> String {
    format!("{}/{page}", Kit::pages(workspace))
}

/// `POST …/append` as `token`.
async fn append(
    kit: &Kit,
    token: &str,
    workspace: WorkspaceId,
    page: &str,
    body: Value,
) -> campbooks_kernel::testing::TestResponse {
    kit.post(
        token,
        &format!("{}/append", page_uri(workspace, page)),
        Some(body),
    )
    .await
}

async fn fixture(db: PgPool) -> (Kit, WorkspaceId, String, String) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, _) = kit.member("ana", workspace);
    let page = kit
        .add(&ana, workspace, json!({"title": "Client onboarding"}))
        .await;
    let id = page["id"].as_str().unwrap().to_owned();
    (kit, workspace, ana, id)
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_writer_adds_a_section_and_the_page_reads_it_back(db: PgPool) {
    let (kit, workspace, ana, page) = fixture(db).await;
    // What Ana already wrote.
    let client = docs::client(31);
    let first = docs::paragraph(&client, "Ana wrote this");
    assert_eq!(
        kit.sync(&ana, workspace, &page, &client, Some(&first))
            .await
            .status,
        StatusCode::OK
    );

    let added = append(
        &kit,
        &ana,
        workspace,
        &page,
        json!({
            "markdown": "## Kickoff\n\n- send the agenda\n- book the room",
            "sources": [
                {"type": "conversations.conversation", "id": THREAD},
                {"type": "tasks.task", "id": TASK},
                {"type": "tasks.task", "id": TASK},
            ],
        }),
    )
    .await;
    assert_eq!(added.status, StatusCode::OK, "{:?}", added.json);
    // A heading and two items: the line of sources is not one of them.
    assert_eq!(added.json["blocks"], 3);
    assert!(added.json["append_id"].as_str().is_some());
    assert_eq!(added.json["revision"], 3);

    assert_eq!(
        kit.page_text(&ana, workspace, &page).await,
        format!(
            "# Client onboarding\n\nAna wrote this\n\n## Kickoff\n\n- send the agenda\n- book the room\n\n\
             [conversations.conversation:{THREAD}] [tasks.task:{TASK}]"
        )
    );

    // An editor that pulls gets Ana's line and the section.
    let pull = kit.sync(&ana, workspace, &page, &client, None).await;
    docs::apply(
        &client,
        &STANDARD
            .decode(pull.json["update"].as_str().unwrap())
            .unwrap(),
    );
    let seen = docs::text(&client);
    assert!(
        seen.contains("Ana wrote this") && seen.contains("book the room"),
        "{seen}"
    );

    // The page announces a body change, with no word of it.
    let events = kit.events("pages.page_changed").await;
    let last = events.last().unwrap();
    assert_eq!(last["kinds"], json!(["body"]));
    assert!(!last.to_string().contains("agenda"));

    // The sources are links: the thread lists the page as a backlink.
    let linked: Vec<(String, String)> = sqlx::query_as(
        "SELECT ref_type, kind FROM pages.links WHERE page_id = $1 ORDER BY ref_type",
    )
    .bind(page.parse::<uuid::Uuid>().unwrap())
    .fetch_all(&kit.db)
    .await
    .unwrap();
    assert_eq!(
        linked,
        [
            (
                "conversations.conversation".to_owned(),
                "mention".to_owned()
            ),
            ("tasks.task".to_owned(), "mention".to_owned()),
        ]
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_reader_cannot_add_and_an_outsider_cannot_see_the_page(db: PgPool) {
    let (kit, workspace, ana, page) = fixture(db).await;
    let (rui, rui_id) = kit.member("rui", workspace);
    let (wes, _) = kit.owner("wes", workspace);
    kit.share(&ana, workspace, &page, rui_id, "reader").await;

    let body = json!({"markdown": "Rui was here"});
    let forbidden = append(&kit, &rui, workspace, &page, body.clone()).await;
    assert_eq!(forbidden.status, StatusCode::FORBIDDEN);
    // A private page: not even its workspace's owner sees it.
    let unseen = append(&kit, &wes, workspace, &page, body).await;
    assert_eq!(unseen.status, StatusCode::NOT_FOUND);
    assert!(
        !kit.page_text(&ana, workspace, &page)
            .await
            .contains("Rui was here")
    );
    let updates: i64 = sqlx::query_scalar("SELECT count(*) FROM pages.updates")
        .fetch_one(&kit.db)
        .await
        .unwrap();
    assert_eq!(updates, 0);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn links_keep_only_safe_schemes_and_html_is_text(db: PgPool) {
    let (kit, workspace, ana, page) = fixture(db).await;
    let added = append(
        &kit,
        &ana,
        workspace,
        &page,
        json!({"markdown":
            "[click](javascript:alert(1)) <script>alert(1)</script> [ok](https://acme.example/a) \
             [mail](mailto:ana@example.com) [data](data:text/html;base64,AAAA)"}),
    )
    .await;
    assert_eq!(added.status, StatusCode::OK, "{:?}", added.json);
    let read = kit.page_text(&ana, workspace, &page).await;
    assert!(
        !read.contains("javascript:") && !read.contains("data:text"),
        "{read}"
    );
    assert!(read.contains("<script>alert(1)</script>"), "{read}");
    assert!(read.contains("[ok](https://acme.example/a)"), "{read}");
    assert!(read.contains("[mail](mailto:ana@example.com)"), "{read}");
    // What was stored passed the one checker: a fresh editor can open it.
    let got = kit.get(&ana, &page_uri(workspace, &page)).await;
    assert_eq!(got.status, StatusCode::OK);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn what_cannot_be_added_is_refused_and_nothing_is_stored(db: PgPool) {
    let (kit, workspace, ana, page) = fixture(db).await;
    let refused = |what: &str, body: Value, status: StatusCode, code: &str| {
        let (kit, ana, page, what, code) = (&kit, &ana, &page, what.to_owned(), code.to_owned());
        async move {
            let answer = append(kit, ana, workspace, page, body).await;
            assert_eq!(answer.status, status, "{what}: {:?}", answer.json);
            assert_eq!(answer.json["code"], code, "{what}: {:?}", answer.json);
        }
    };
    refused(
        "too long",
        json!({"markdown": "x".repeat(MAX_MARKDOWN_BYTES + 1)}),
        StatusCode::PAYLOAD_TOO_LARGE,
        "markdown_too_large",
    )
    .await;
    refused(
        "empty",
        json!({"markdown": " \n\n "}),
        StatusCode::BAD_REQUEST,
        "markdown_empty",
    )
    .await;
    refused(
        "a source a page cannot point at",
        json!({"markdown": "x", "sources": [{"type": "documents.document", "id": THREAD}]}),
        StatusCode::BAD_REQUEST,
        "invalid_ref",
    )
    .await;
    refused(
        "a source that is not an id",
        json!({"markdown": "x", "sources": [{"type": "tasks.task", "id": "not-an-id"}]}),
        StatusCode::BAD_REQUEST,
        "invalid_ref",
    )
    .await;
    let many: Vec<Value> = (0..11)
        .map(|n| json!({"type": "tasks.task", "id": format!("0192f0c4-7d4e-7b39-a7f1-3a8f0e2b9c{n:02}")}))
        .collect();
    refused(
        "too many sources",
        json!({"markdown": "x", "sources": many}),
        StatusCode::BAD_REQUEST,
        "too_many_sources",
    )
    .await;
    let unknown = json!({"markdown": "x", "colour": "red"});
    assert_eq!(
        append(&kit, &ana, workspace, &page, unknown).await.status,
        StatusCode::BAD_REQUEST
    );
    let updates: i64 = sqlx::query_scalar("SELECT count(*) FROM pages.updates")
        .fetch_one(&kit.db)
        .await
        .unwrap();
    assert_eq!(updates, 0);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_page_in_the_trash_or_a_table_takes_nothing(db: PgPool) {
    let (kit, workspace, ana, page) = fixture(db).await;
    let table = kit.table(&ana, workspace).await;
    let table_id = table["page"]["id"].as_str().unwrap();
    let answer = append(&kit, &ana, workspace, table_id, json!({"markdown": "x"})).await;
    assert_eq!(answer.status, StatusCode::CONFLICT);
    assert_eq!(answer.json["code"], "not_a_page");

    kit.post(&ana, &format!("{}/trash", page_uri(workspace, &page)), None)
        .await;
    let answer = append(&kit, &ana, workspace, &page, json!({"markdown": "x"})).await;
    assert_eq!(answer.status, StatusCode::CONFLICT);
    assert_eq!(answer.json["code"], "page_trashed");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_new_routes_are_not_there_while_mail_and_scout_are_off(db: PgPool) {
    let (kit, workspace, ana, page) = fixture(db).await;
    let kit = kit.with_mail_scout(false);
    let base = page_uri(workspace, &page);
    let attachment = uuid::Uuid::now_v7();
    for (method, uri, body) in [
        (
            "POST",
            format!("{base}/append"),
            Some(json!({"markdown": "x"})),
        ),
        (
            "POST",
            format!("{base}/unappend"),
            Some(json!({"append_id": attachment})),
        ),
        ("GET", format!("{base}/text"), None),
        (
            "POST",
            format!("{base}/files/from-message"),
            Some(
                json!({"conversation": attachment, "message": attachment, "attachments": [attachment]}),
            ),
        ),
        (
            "POST",
            format!("{base}/files/detach"),
            Some(json!({"files": ["a".repeat(64)]})),
        ),
    ] {
        let answer = if method == "GET" {
            kit.get(&ana, &uri).await
        } else {
            kit.post(&ana, &uri, body).await
        };
        assert_eq!(answer.status, StatusCode::NOT_FOUND, "{method} {uri}");
    }
    // A page made with Markdown is not made, and nothing else changes.
    let made = kit
        .post(
            &ana,
            &Kit::pages(workspace),
            Some(json!({"title": "From Markdown", "markdown": "x"})),
        )
        .await;
    assert_eq!(made.status, StatusCode::NOT_FOUND);
    let plain = kit
        .post(
            &ana,
            &Kit::pages(workspace),
            Some(json!({"title": "Plain"})),
        )
        .await;
    assert_eq!(plain.status, StatusCode::CREATED);
    assert_eq!(kit.page_count(workspace).await, 2);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_append_to_a_page_kept_in_step_with_notion_reaches_notion_like_any_edit(db: PgPool) {
    use crate::domain::notion::tests::{id, para, snapshot};
    let rig = Rig::new(db).await;
    let notion_page = id(0xAA);
    let page = rig.import(notion_page, "Roadmap").await;
    let mut snap = snapshot(vec![para(1, "Plan the launch", 1)], 5, true);
    snap.notion_page = notion_page;
    rig.read(&page, notion_page, &snap).await;

    let added = append(
        &rig.kit,
        &rig.token,
        rig.ws,
        &page,
        json!({"markdown": "## Kickoff\n\n- send the agenda"}),
    )
    .await;
    assert_eq!(added.status, StatusCode::OK, "{:?}", added.json);
    rig.push_now().await;

    let requests = rig.requests(&page).await;
    assert_eq!(requests.len(), 1, "{requests:?}");
    let items = requests[0].2.as_array().unwrap();
    assert_eq!(items.len(), 2, "{items:?}");
    assert!(
        items.iter().all(|item| item["action"] == "append"),
        "{items:?}"
    );
    // Ids, kinds and hashes: not a word of it.
    assert!(!requests[0].2.to_string().contains("agenda"));
    let events = rig.kit.events("pages.notion_changes_requested").await;
    assert_eq!(events.len(), 1);
    let document = rig.blob_json(&events[0]["body"]).await;
    let sent = document.to_string();
    assert!(
        sent.contains("Kickoff") && sent.contains("send the agenda"),
        "{sent}"
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_append_is_tracked_with_ids_and_buckets_only(db: PgPool) {
    use campbooks_kernel::testing;
    let kit = Kit::new(db).await.tracking();
    let workspace = WorkspaceId::new();
    let (ana, _) = kit.member("ana", workspace);
    let page = kit
        .add(&ana, workspace, json!({"title": "Secret plan"}))
        .await;
    let id = page["id"].as_str().unwrap();
    let added = append(
        &kit,
        &ana,
        workspace,
        id,
        json!({"markdown": "Secret words", "sources": [{"type": "tasks.task", "id": TASK}]}),
    )
    .await;
    assert_eq!(added.status, StatusCode::OK, "{:?}", added.json);
    // The same words again are a second append; a refused one tracks nothing.
    append(&kit, &ana, workspace, id, json!({"markdown": ""})).await;

    let tracked = testing::tracked_events(&kit.db, "pages.section_appended").await;
    assert_eq!(tracked.len(), 1);
    let event = &tracked[0];
    assert_eq!(event.envelope["operation"], "pages.append");
    assert_eq!(event.envelope["actor"]["type"], "user");
    assert_eq!(event.body["page"]["id"], page["id"]);
    // One block (the sources are counted apart), one source.
    assert_eq!(event.body["blocks"], "one");
    assert_eq!(event.body["sources"], "one");
    let body = event.body.to_string();
    for text in ["Secret", TASK, "\"title\":"] {
        assert!(!body.contains(text), "{body}");
    }
}
