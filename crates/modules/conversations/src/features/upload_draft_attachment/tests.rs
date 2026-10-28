use axum::http::StatusCode;
use campbooks_kernel::testing::TestResponse;
use serde_json::{Value, json};
use sqlx::PgPool;

use crate::{model::InboxRole, testkit::Kit};

/// A new draft for Sam, as `GET` reads it: its URI and what it shows.
async fn draft(
    kit: &Kit,
    (workspace, inbox): (impl std::fmt::Display, impl std::fmt::Display),
    token: &str,
) -> (String, Value) {
    let uri = format!("/api/v1/w/{workspace}/drafts");
    let created = kit
        .post(
            token,
            &uri,
            Some(
                json!({"kind": "new", "inbox_id": inbox.to_string(), "subject": "Lease",
                "to": [{"address": "leo@example.com"}], "body": "Attached."}),
            ),
        )
        .await;
    assert_eq!(created.status, StatusCode::CREATED, "{:?}", created.json);
    let uri = format!("{uri}/{}", created.json["id"].as_str().unwrap());
    let shown = kit.get(token, &uri).await.json;
    (uri, shown)
}

/// Upload `bytes` named `name` to the draft at `uri` as `shown` showed it.
async fn upload(
    kit: &Kit,
    token: &str,
    uri: &str,
    shown: &Value,
    (name, content_type): (&str, &str),
    bytes: Vec<u8>,
) -> TestResponse {
    let version = shown["version"].as_u64().unwrap();
    let query = format!("version={version}&filename={}", encode(name));
    let token_header = shown["content_token"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    kit.upload(
        token,
        &format!("{uri}/attachments?{query}"),
        content_type,
        bytes,
        &[("x-content-token", &token_header)],
    )
    .await
}

fn encode(name: &str) -> String {
    name.bytes()
        .map(|b| match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

fn pdf(size: usize) -> Vec<u8> {
    let mut bytes = b"%PDF-1.7\n".to_vec();
    bytes.resize(size, b'x');
    bytes
}

/// Blobs the draft holds, by owner reference.
async fn held(kit: &Kit, uri: &str) -> i64 {
    let id: sqlx::types::Uuid = uri.rsplit('/').next().unwrap().parse().unwrap();
    sqlx::query_scalar("SELECT count(*) FROM kernel.blob_refs WHERE owner_id = $1")
        .bind(id)
        .fetch_one(&kit.db)
        .await
        .unwrap()
}

async fn blobs(kit: &Kit) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM kernel.blobs")
        .fetch_one(&kit.db)
        .await
        .unwrap()
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_pdf_is_attached_and_the_version_goes_up(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, inbox, sam) = kit.sams_inbox().await;
    let (uri, shown) = draft(&kit, (workspace, inbox), &sam).await;
    let before = blobs(&kit).await;
    let bytes = pdf(400 * 1024);
    let up = upload(
        &kit,
        &sam,
        &uri,
        &shown,
        ("lease-4b.pdf", "Application/PDF"),
        bytes.clone(),
    )
    .await;
    assert_eq!(up.status, StatusCode::CREATED, "{:?}", up.json);
    let file = &up.json["attachments"][0];
    assert_eq!(file["filename"], "lease-4b.pdf");
    assert_eq!(file["source"], "uploaded");
    assert_eq!(file["size"], bytes.len());
    assert_eq!(file["content_type"], "application/pdf");
    assert_eq!(up.json["version"], shown["version"].as_u64().unwrap() + 1);
    assert_ne!(up.json["content_token"], shown["content_token"]);
    assert_eq!(blobs(&kit).await, before + 1);
    assert_eq!(held(&kit, &uri).await, 1);
    // The draft reads back the same, and the answer's token saves.
    let read = kit.get(&sam, &uri).await.json;
    assert_eq!(read["attachments"], up.json["attachments"]);
    let saved = kit
        .patch(
            &sam,
            &uri,
            json!({"version": up.json["version"], "content_token": up.json["content_token"],
                "body": "Attached, thanks."}),
        )
        .await;
    assert_eq!(saved.status, StatusCode::OK, "{:?}", saved.json);
    assert_eq!(saved.json["attachments"].as_array().unwrap().len(), 1);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn another_member_cannot_upload_to_the_draft(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, inbox, sam) = kit.sams_inbox().await;
    let (uri, shown) = draft(&kit, (workspace, inbox), &sam).await;
    let (rui, rui_id) = kit.member("rui", workspace);
    kit.add_to_inbox(inbox, rui_id, InboxRole::Sender).await;
    let before = blobs(&kit).await;
    let up = upload(
        &kit,
        &rui,
        &uri,
        &shown,
        ("a.pdf", "application/pdf"),
        pdf(10),
    )
    .await;
    assert_eq!(up.status, StatusCode::NOT_FOUND, "{:?}", up.json);
    assert_eq!(blobs(&kit).await, before);
    assert_eq!(kit.get(&sam, &uri).await.json["version"], shown["version"]);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_stale_version_or_token_is_a_conflict(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, inbox, sam) = kit.sams_inbox().await;
    let (uri, shown) = draft(&kit, (workspace, inbox), &sam).await;
    let before = blobs(&kit).await;
    let mut stale = shown.clone();
    stale["version"] = json!(0);
    let up = upload(
        &kit,
        &sam,
        &uri,
        &stale,
        ("a.pdf", "application/pdf"),
        pdf(10),
    )
    .await;
    assert_eq!(up.status, StatusCode::CONFLICT, "{:?}", up.json);
    assert_eq!(up.json["code"], "draft_changed");
    for token in [Value::Null, json!("garbage")] {
        let mut wrong = shown.clone();
        wrong["content_token"] = token;
        let up = upload(
            &kit,
            &sam,
            &uri,
            &wrong,
            ("a.pdf", "application/pdf"),
            pdf(10),
        )
        .await;
        assert_eq!(up.status, StatusCode::CONFLICT, "{:?}", up.json);
        assert_eq!(up.json["code"], "content_changed");
    }
    // A token of an earlier state of the draft.
    let first = upload(
        &kit,
        &sam,
        &uri,
        &shown,
        ("a.pdf", "application/pdf"),
        pdf(10),
    )
    .await;
    assert_eq!(first.status, StatusCode::CREATED);
    let mut old = first.json.clone();
    old["content_token"] = shown["content_token"].clone();
    let up = upload(
        &kit,
        &sam,
        &uri,
        &old,
        ("b.pdf", "application/pdf"),
        pdf(10),
    )
    .await;
    assert_eq!(up.json["code"], "content_changed", "{:?}", up.json);
    assert_eq!(blobs(&kit).await, before + 1, "only the first was stored");
    assert_eq!(held(&kit, &uri).await, 1);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn limits_are_kept(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, inbox, sam) = kit.sams_inbox().await;
    let (uri, mut shown) = draft(&kit, (workspace, inbox), &sam).await;
    let before = blobs(&kit).await;
    let mib = 1024 * 1024;

    // Empty.
    let empty = upload(
        &kit,
        &sam,
        &uri,
        &shown,
        ("a.pdf", "application/pdf"),
        vec![],
    )
    .await;
    assert_eq!(empty.status, StatusCode::BAD_REQUEST);
    assert_eq!(empty.json["code"], "file_empty");
    // One file over the total.
    let big = upload(
        &kit,
        &sam,
        &uri,
        &shown,
        ("big.bin", "application/octet-stream"),
        vec![1; 25 * mib + 1],
    )
    .await;
    assert_eq!(big.status, StatusCode::PAYLOAD_TOO_LARGE, "{:?}", big.json);
    // 24 MiB held, then 2 MiB more.
    let held24 = upload(
        &kit,
        &sam,
        &uri,
        &shown,
        ("a.bin", "application/octet-stream"),
        vec![2; 24 * mib],
    )
    .await;
    assert_eq!(held24.status, StatusCode::CREATED, "{:?}", held24.json);
    shown = held24.json.clone();
    let over = upload(
        &kit,
        &sam,
        &uri,
        &shown,
        ("b.bin", "application/octet-stream"),
        vec![3; 2 * mib],
    )
    .await;
    assert_eq!(
        over.status,
        StatusCode::PAYLOAD_TOO_LARGE,
        "{:?}",
        over.json
    );
    assert_eq!(over.json["code"], "attachments_too_large");
    let read = kit.get(&sam, &uri).await.json;
    assert_eq!(read["attachments"].as_array().unwrap().len(), 1);
    assert_eq!(read["version"], shown["version"]);
    assert_eq!(blobs(&kit).await, before + 1);

    // Exactly the rest fits.
    let fits = upload(
        &kit,
        &sam,
        &uri,
        &shown,
        ("c.bin", "application/octet-stream"),
        vec![4; mib],
    )
    .await;
    assert_eq!(fits.status, StatusCode::CREATED, "{:?}", fits.json);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn twenty_files_at_most(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, inbox, sam) = kit.sams_inbox().await;
    let (uri, mut shown) = draft(&kit, (workspace, inbox), &sam).await;
    for n in 0..20 {
        let up = upload(
            &kit,
            &sam,
            &uri,
            &shown,
            (&format!("f{n}.txt"), "text/plain"),
            format!("file {n}").into_bytes(),
        )
        .await;
        assert_eq!(up.status, StatusCode::CREATED, "{n}: {:?}", up.json);
        shown = up.json;
    }
    let before = blobs(&kit).await;
    let one_more = upload(
        &kit,
        &sam,
        &uri,
        &shown,
        ("f20.txt", "text/plain"),
        b"x".to_vec(),
    )
    .await;
    assert_eq!(
        one_more.status,
        StatusCode::BAD_REQUEST,
        "{:?}",
        one_more.json
    );
    assert_eq!(one_more.json["code"], "too_many_attachments");
    assert_eq!(blobs(&kit).await, before);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn blocked_types_and_odd_names(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, inbox, sam) = kit.sams_inbox().await;
    let (uri, shown) = draft(&kit, (workspace, inbox), &sam).await;
    let before = blobs(&kit).await;
    for name in ["setup.EXE", "run.bat", "x.Js", "dir/evil.scr", "evil.exe. "] {
        let up = upload(
            &kit,
            &sam,
            &uri,
            &shown,
            (name, "text/plain"),
            b"x".to_vec(),
        )
        .await;
        assert_eq!(up.status, StatusCode::BAD_REQUEST, "{name}: {:?}", up.json);
        assert_eq!(up.json["code"], "attachment_blocked", "{name}");
    }
    for name in ["", "...", "dir/", "\"\""] {
        let up = upload(
            &kit,
            &sam,
            &uri,
            &shown,
            (name, "text/plain"),
            b"x".to_vec(),
        )
        .await;
        assert_eq!(
            up.status,
            StatusCode::BAD_REQUEST,
            "{name:?}: {:?}",
            up.json
        );
        assert_eq!(up.json["code"], "filename_missing", "{name:?}");
    }
    assert_eq!(blobs(&kit).await, before);

    // A path in the name is dropped; a bad type is not trusted.
    let up = upload(
        &kit,
        &sam,
        &uri,
        &shown,
        ("../../etc/passwd", "text/ html; <script>"),
        b"root".to_vec(),
    )
    .await;
    assert_eq!(up.status, StatusCode::CREATED, "{:?}", up.json);
    assert_eq!(up.json["attachments"][0]["filename"], "passwd");
    assert_eq!(
        up.json["attachments"][0]["content_type"],
        "application/octet-stream"
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_draft_on_its_way_takes_no_files(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, inbox, sam) = kit.sams_inbox().await;
    let (uri, shown) = draft(&kit, (workspace, inbox), &sam).await;
    let sent = kit
        .post(
            &sam,
            &format!("{uri}/send"),
            Some(json!({"version": shown["version"], "content_token": shown["content_token"]})),
        )
        .await;
    assert_eq!(sent.status, StatusCode::ACCEPTED, "{:?}", sent.json);
    let before = blobs(&kit).await;
    let up = upload(
        &kit,
        &sam,
        &uri,
        &sent.json,
        ("a.pdf", "application/pdf"),
        pdf(10),
    )
    .await;
    assert_eq!(up.status, StatusCode::CONFLICT, "{:?}", up.json);
    assert_eq!(up.json["code"], "draft_not_editable");
    assert_eq!(blobs(&kit).await, before);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_reader_of_the_inbox_cannot_upload(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, inbox, sam) = kit.sams_inbox().await;
    let (uri, shown) = draft(&kit, (workspace, inbox), &sam).await;
    // Sam keeps the draft but is now only a reader of its inbox.
    sqlx::query("UPDATE conversations.inbox_members SET role = 'reader' WHERE inbox_id = $1")
        .bind(inbox)
        .execute(&kit.db)
        .await
        .unwrap();
    let up = upload(
        &kit,
        &sam,
        &uri,
        &shown,
        ("a.pdf", "application/pdf"),
        pdf(10),
    )
    .await;
    assert!(
        matches!(up.status, StatusCode::FORBIDDEN | StatusCode::NOT_FOUND),
        "{:?}",
        up.json
    );
    assert_eq!(held(&kit, &uri).await, 0);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn removing_an_uploaded_file_releases_it(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, inbox, sam) = kit.sams_inbox().await;
    let (uri, shown) = draft(&kit, (workspace, inbox), &sam).await;
    let a = upload(
        &kit,
        &sam,
        &uri,
        &shown,
        ("a.txt", "text/plain"),
        b"aaa".to_vec(),
    )
    .await;
    let b = upload(
        &kit,
        &sam,
        &uri,
        &a.json,
        ("b.txt", "text/plain"),
        b"bbb".to_vec(),
    )
    .await;
    assert_eq!(b.status, StatusCode::CREATED, "{:?}", b.json);
    assert_eq!(held(&kit, &uri).await, 2);
    let keep = b.json["attachments"][1]["id"].clone();
    let saved = kit
        .patch(
            &sam,
            &uri,
            json!({"version": b.json["version"], "content_token": b.json["content_token"],
                "attachments": [keep]}),
        )
        .await;
    assert_eq!(saved.status, StatusCode::OK, "{:?}", saved.json);
    assert_eq!(saved.json["attachments"].as_array().unwrap().len(), 1);
    assert_eq!(saved.json["attachments"][0]["filename"], "b.txt");
    assert_eq!(
        held(&kit, &uri).await,
        1,
        "the dropped file is no longer held"
    );
    // Then none.
    let none = kit
        .patch(
            &sam,
            &uri,
            json!({"version": saved.json["version"], "content_token": saved.json["content_token"],
                "attachments": []}),
        )
        .await;
    assert_eq!(none.json["attachments"], json!([]));
    assert_eq!(held(&kit, &uri).await, 0);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn discarding_the_draft_releases_uploaded_files(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, inbox, sam) = kit.sams_inbox().await;
    let (uri, shown) = draft(&kit, (workspace, inbox), &sam).await;
    let up = upload(
        &kit,
        &sam,
        &uri,
        &shown,
        ("a.txt", "text/plain"),
        b"aaa".to_vec(),
    )
    .await;
    assert_eq!(up.status, StatusCode::CREATED);
    assert_eq!(held(&kit, &uri).await, 1);
    let gone = kit.delete(&sam, &uri).await;
    assert_eq!(gone.status, StatusCode::NO_CONTENT);
    assert_eq!(held(&kit, &uri).await, 0);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn sending_carries_the_uploaded_file(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, inbox, sam) = kit.sams_inbox().await;
    let (uri, shown) = draft(&kit, (workspace, inbox), &sam).await;
    let up = upload(
        &kit,
        &sam,
        &uri,
        &shown,
        ("lease-4b.pdf", "application/pdf"),
        pdf(2048),
    )
    .await;
    assert_eq!(up.status, StatusCode::CREATED, "{:?}", up.json);
    let sent = kit
        .post(
            &sam,
            &format!("{uri}/send"),
            Some(json!({"version": up.json["version"], "content_token": up.json["content_token"]})),
        )
        .await;
    assert_eq!(sent.status, StatusCode::ACCEPTED, "{:?}", sent.json);
    sqlx::query(
        "UPDATE kernel.jobs SET run_at = now() WHERE kind = 'conversations.release_send' AND state = 'pending'",
    )
    .execute(&kit.db)
    .await
    .unwrap();
    kit.run_jobs().await;
    let id: sqlx::types::Uuid = uri.rsplit('/').next().unwrap().parse().unwrap();
    let (document, stored): (Value, Value) =
        sqlx::query_as("SELECT send_document, attachments FROM conversations.drafts WHERE id = $1")
            .bind(id)
            .fetch_one(&kit.db)
            .await
            .unwrap();
    let document: campbooks_kernel::blobs::BlobRef = serde_json::from_value(document).unwrap();
    let outgoing: Value = serde_json::from_str(&kit.read(workspace, &document).await).unwrap();
    let files = outgoing["attachments"].as_array().unwrap();
    assert_eq!(files.len(), 1, "{outgoing}");
    assert_eq!(files[0]["filename"], "lease-4b.pdf");
    assert_eq!(files[0]["blob"], stored[0]["blob"]);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_assistant_cannot_attach_files(db: PgPool) {
    use campbooks_kernel::operations::InvokeError;
    let kit = Kit::new(db).await;
    let (workspace, inbox, sam) = kit.sams_inbox().await;
    let (uri, shown) = draft(&kit, (workspace, inbox), &sam).await;
    let id = uri.rsplit('/').next().unwrap();
    for confirmed in [false, true] {
        let result = kit
            .as_assistant(
                "sam",
                workspace,
                (
                    "conversations.upload_draft_attachment",
                    json!({"draft_id": id, "version": shown["version"], "filename": "a.pdf"}),
                ),
                confirmed,
            )
            .await;
        assert!(
            matches!(result, Err(InvokeError::NotAllowed { .. })),
            "{result:?}"
        );
    }
    assert_eq!(held(&kit, &uri).await, 0);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_upload_is_tracked_as_a_change_without_the_file_name(db: PgPool) {
    let kit = Kit::new(db).await.tracking();
    let (workspace, inbox, sam) = kit.sams_inbox().await;
    let (uri, shown) = draft(&kit, (workspace, inbox), &sam).await;
    let up = upload(
        &kit,
        &sam,
        &uri,
        &shown,
        ("lease-4b.pdf", "application/pdf"),
        pdf(64),
    )
    .await;
    assert_eq!(up.status, StatusCode::CREATED, "{:?}", up.json);
    let events =
        campbooks_kernel::testing::tracked_events(&kit.db, "conversations.draft_updated").await;
    assert_eq!(events.len(), 1);
    assert_eq!(
        events[0].envelope["operation"],
        "conversations.upload_draft_attachment"
    );
    assert!(!events[0].body.to_string().contains("lease-4b"));
}

/// Where every inbox sends from in these tests: a mailbox that carries at
/// most `max` bytes of attachments (as Microsoft Graph does).
#[derive(Debug)]
struct Carries(u64);

impl campbooks_kernel::outbound::OutboundDestinations for Carries {
    fn destination<'a>(
        &'a self,
        _conn: &'a mut sqlx::PgConnection,
        _inbox: sqlx::types::Uuid,
    ) -> campbooks_kernel::outbound::DestinationFuture<'a> {
        let max = self.0;
        Box::pin(async move {
            Ok(Some(campbooks_kernel::outbound::Resolved {
                destination: campbooks_kernel::outbound::Destination {
                    connection: sqlx::types::Uuid::nil(),
                    stream: sqlx::types::Uuid::nil(),
                    mailbox: "inbox".into(),
                },
                as_at_deploy: true,
                max_attachment_bytes: Some(max),
            }))
        })
    }
}

/// A mailbox that carries less than 25 MiB (Microsoft Graph): a file past
/// its limit is refused at upload, naming it, and the inbox says its limit.
#[sqlx::test(migrations = "../../../migrations")]
async fn a_mailbox_that_carries_less_refuses_the_file_up_front(db: PgPool) {
    let kit = Kit::new(db)
        .await
        .with_destinations(std::sync::Arc::new(Carries(2_100_000)));
    let (workspace, inbox, sam) = kit.sams_inbox().await;
    let (uri, shown) = draft(&kit, (workspace, inbox), &sam).await;
    let refused = upload(
        &kit,
        &sam,
        &uri,
        &shown,
        ("plans.pdf", "application/pdf"),
        vec![1; 2_200_000],
    )
    .await;
    assert_eq!(
        refused.status,
        StatusCode::PAYLOAD_TOO_LARGE,
        "{:?}",
        refused.json
    );
    assert_eq!(refused.json["code"], "attachments_too_large_for_mailbox");
    assert!(
        refused.json["detail"]
            .as_str()
            .unwrap_or_default()
            .contains("2.1 MB"),
        "{:?}",
        refused.json
    );
    // A smaller one goes.
    let taken = upload(
        &kit,
        &sam,
        &uri,
        &shown,
        ("plans.pdf", "application/pdf"),
        vec![1; 1_000_000],
    )
    .await;
    assert_eq!(taken.status, StatusCode::CREATED, "{:?}", taken.json);
    // The inbox says what it carries.
    let inboxes = kit
        .get(&sam, &format!("/api/v1/w/{workspace}/inboxes"))
        .await;
    assert_eq!(
        inboxes.json["inboxes"][0]["max_attachment_bytes"],
        2_100_000
    );
}

/// A client token with no member behind it attaches nothing: a draft is a
/// member's, and one with no author must not match a caller with none.
#[sqlx::test(migrations = "../../../migrations")]
async fn a_caller_with_no_member_cannot_upload(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, inbox, sam) = kit.sams_inbox().await;
    let (uri, shown) = draft(&kit, (workspace, inbox), &sam).await;
    let before = blobs(&kit).await;
    let bare = kit.client(workspace, "conversations:read conversations:send");
    let refused = upload(
        &kit,
        &bare,
        &uri,
        &shown,
        ("a.txt", "text/plain"),
        b"x".to_vec(),
    )
    .await;
    assert_eq!(refused.status, StatusCode::NOT_FOUND, "{:?}", refused.json);
    assert_eq!(blobs(&kit).await, before);
}

/// A body past 25 MiB is refused as a problem naming the limit, never the
/// framework's plain-text 413.
#[sqlx::test(migrations = "../../../migrations")]
async fn a_body_past_the_limit_is_a_problem(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, inbox, sam) = kit.sams_inbox().await;
    let (uri, shown) = draft(&kit, (workspace, inbox), &sam).await;
    let before = blobs(&kit).await;
    let refused = upload(
        &kit,
        &sam,
        &uri,
        &shown,
        ("big.bin", "application/octet-stream"),
        vec![0; 25 * 1024 * 1024 + 1],
    )
    .await;
    assert_eq!(
        refused.status,
        StatusCode::PAYLOAD_TOO_LARGE,
        "{:?}",
        refused.json
    );
    assert_eq!(refused.json["code"], "attachments_too_large");
    assert_eq!(blobs(&kit).await, before);
}
