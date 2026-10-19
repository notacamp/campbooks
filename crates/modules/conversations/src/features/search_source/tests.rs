use campbooks_kernel::{
    RecordRef,
    db::BeginFor as _,
    search::{Facet, Group, PersonRole, SearchSource as _, SourceContext, facet},
    tenancy::WorkspaceId,
};
use sqlx::PgPool;

use super::ConversationsSource;
use crate::{
    events::{INBOX_RECORD, MESSAGE_RECORD},
    model::InboxRole,
    testkit::Kit,
};

async fn group_of(kit: &Kit, workspace: WorkspaceId, id: &str) -> Option<Group> {
    let mut tx = kit.state.db.begin_for(workspace).await.unwrap();
    let cx = SourceContext {
        conn: &mut tx,
        blobs: &kit.state.blobs,
        workspace,
    };
    let group = ConversationsSource.group(cx, id).await.unwrap();
    tx.commit().await.unwrap();
    group
}

async fn texts_of(
    kit: &Kit,
    workspace: WorkspaceId,
    entries: &[RecordRef],
) -> Vec<campbooks_kernel::search::EntryText> {
    let mut tx = kit.state.db.begin_for(workspace).await.unwrap();
    let cx = SourceContext {
        conn: &mut tx,
        blobs: &kit.state.blobs,
        workspace,
    };
    let texts = ConversationsSource.texts(cx, entries).await.unwrap();
    tx.commit().await.unwrap();
    texts
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_conversation_with_its_messages(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, inbox, _) = kit.sams_inbox().await;
    let id = kit.conversation_of("en-1").await;
    let group = group_of(&kit, workspace, &id).await.unwrap();
    assert_eq!(group.container, RecordRef::new(INBOX_RECORD, &inbox));
    assert_eq!(group.title, "Q4 plan: can you review by Friday?");
    assert_eq!(group.keywords, vec!["Clients".to_owned()]);
    assert_eq!(group.facets[facet::UNREAD], Facet::Bool(true));
    assert_eq!(group.facets[facet::COUNT], Facet::Count(2));
    assert_eq!(group.entries.len(), 2);
    let first = &group.entries[0];
    assert_eq!(first.facets[facet::MAILBOX], Facet::Text("inbox".into()));
    assert_eq!(first.facets[facet::ATTACHMENTS], Facet::Count(1));
    assert_eq!(
        first.facets[facet::DIRECTION],
        Facet::Text("inbound".into())
    );

    let records: Vec<RecordRef> = group.entries.iter().map(|e| e.record.clone()).collect();
    let texts = texts_of(&kit, workspace, &records).await;
    let maya = texts
        .iter()
        .find(|t| t.record == records[0])
        .expect("the first message's text");
    assert!(maya.text.starts_with("Hi Sam,"), "{}", maya.text);
    assert_eq!(maya.attachments, vec!["q4-plan.pdf".to_owned()]);
    let from = &maya.people[0];
    assert_eq!(from.role, PersonRole::From);
    assert_eq!(from.address, "maya@northwind-studio.example");
    assert!(maya.people.iter().any(|p| p.role == PersonRole::To));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_flag_change_keeps_the_stamps(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, _, sam) = kit.sams_inbox().await;
    let id = kit.conversation_of("en-1").await;
    let before = group_of(&kit, workspace, &id).await.unwrap();
    kit.post(
        &sam,
        &format!("/api/v1/w/{workspace}/conversations/{id}/read"),
        None,
    )
    .await;
    kit.post(
        &sam,
        &format!("/api/v1/w/{workspace}/conversations/{id}/star"),
        None,
    )
    .await;
    let after = group_of(&kit, workspace, &id).await.unwrap();
    assert_eq!(after.facets[facet::UNREAD], Facet::Bool(false));
    assert_eq!(after.facets[facet::STARRED], Facet::Bool(true));
    let stamps =
        |group: &Group| -> Vec<String> { group.entries.iter().map(|e| e.stamp.clone()).collect() };
    assert_eq!(stamps(&before), stamps(&after));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn gone_and_unknown_conversations(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, _, _) = kit.sams_inbox().await;
    assert!(group_of(&kit, workspace, "not-an-id").await.is_none());
    assert!(
        group_of(
            &kit,
            workspace,
            &crate::model::ConversationId::new().to_string()
        )
        .await
        .is_none()
    );
    // Another workspace's conversation is invisible under row-level security.
    let id = kit.conversation_of("en-1").await;
    assert!(group_of(&kit, WorkspaceId::new(), &id).await.is_none());
    let texts = texts_of(
        &kit,
        workspace,
        &[RecordRef::new(
            MESSAGE_RECORD,
            &crate::model::MessageId::new(),
        )],
    )
    .await;
    assert!(texts.is_empty());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn pages_list_every_conversation_once(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (_, sam) = kit.member("sam", workspace);
    let inbox = kit
        .inbox(workspace, "sam@example.com", &[(sam, InboxRole::Manager)])
        .await;
    for name in ["en", "pt-PT", "fr"] {
        kit.ingest(workspace, inbox, name).await;
    }
    let mut seen = Vec::new();
    let mut after: Option<String> = None;
    loop {
        let mut tx = kit.state.db.begin_for(workspace).await.unwrap();
        let cx = SourceContext {
            conn: &mut tx,
            blobs: &kit.state.blobs,
            workspace,
        };
        let page = ConversationsSource
            .page(cx, after.as_deref(), 3)
            .await
            .unwrap();
        tx.commit().await.unwrap();
        seen.extend(page.groups);
        match page.next {
            Some(next) => after = Some(next),
            None => break,
        }
    }
    let total: i64 = sqlx::query_scalar("SELECT count(*) FROM conversations.conversations")
        .fetch_one(&kit.db)
        .await
        .unwrap();
    let mut unique = seen.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), seen.len());
    assert_eq!(i64::try_from(seen.len()).unwrap(), total);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn mail_with_only_a_head_and_no_body_tag_is_read_for_search(db: PgPool) {
    // campbooks-cloud#218: HTML that opens with `<head><meta>` and never
    // says `<body>` still has words; its stamp says how HTML was read, so
    // a better reading reads it again.
    let kit = Kit::new(db).await;
    let (workspace, inbox, _) = kit.sams_inbox().await;
    kit.ingest_value(
        workspace,
        inbox,
        &serde_json::json!({"changes": [{"op": "upsert", "message": {
            "provider_message_id": "head-only-1",
            "from": {"name": "Acme Hosting", "address": "billing@acme-hosting.example"},
            "to": [{"address": "sam@example.com"}],
            "subject": "Invoice INV-2041",
            "sent_at": "2026-10-13T06:30:00Z",
            "direction": "inbound",
            "html": "<head><meta charset=utf-8><title>Secret title</title>\
                     <style>p { color: red }</style><p>Invoice due Friday",
            "mailbox": {"in_inbox": true},
            "read": false, "starred": false
        }}]}),
    )
    .await;
    let id = kit.conversation_of("head-only-1").await;
    let group = group_of(&kit, workspace, &id).await.unwrap();
    assert!(
        group.entries[0].stamp.ends_with(":h2"),
        "{}",
        group.entries[0].stamp
    );
    let records: Vec<RecordRef> = group.entries.iter().map(|e| e.record.clone()).collect();
    let texts = texts_of(&kit, workspace, &records).await;
    let text = &texts[0].text;
    assert!(text.contains("Invoice due Friday"), "{text:?}");
    assert!(
        !text.contains("Secret") && !text.contains("color"),
        "{text:?}"
    );
    // The thread of en-1 (text and HTML) and en-2 (text alone): only the
    // message with HTML carries how HTML is read.
    let en = kit.conversation_of("en-1").await;
    let group = group_of(&kit, workspace, &en).await.unwrap();
    let stamps: Vec<bool> = group
        .entries
        .iter()
        .map(|e| e.stamp.ends_with(":h2"))
        .collect();
    assert_eq!(stamps, [true, false]);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_text_part_of_blanks_only_falls_back_to_the_html(db: PgPool) {
    // As stored before campbooks-cloud#218: the connector's parser made a
    // text part of three line breaks out of HTML it could not read.
    let kit = Kit::new(db).await;
    let (workspace, inbox, _) = kit.sams_inbox().await;
    kit.ingest_value(
        workspace,
        inbox,
        &serde_json::json!({"changes": [{"op": "upsert", "message": {
            "provider_message_id": "blank-text-1",
            "from": {"name": "Quay Office", "address": "office@quay-moorings.example"},
            "to": [{"address": "sam@example.com"}],
            "subject": "Your berth for next season",
            "sent_at": "2026-10-13T06:30:00Z",
            "direction": "inbound",
            "text": "\n\n\n",
            "html": "<head><meta charset=utf-8><p>Your berth at the harbourlight pontoon is kept.",
            "mailbox": {"in_inbox": true},
            "read": false, "starred": false
        }}]}),
    )
    .await;
    let id = kit.conversation_of("blank-text-1").await;
    let group = group_of(&kit, workspace, &id).await.unwrap();
    assert!(group.entries[0].stamp.ends_with(":h2"));
    let records: Vec<RecordRef> = group.entries.iter().map(|e| e.record.clone()).collect();
    let texts = texts_of(&kit, workspace, &records).await;
    assert!(
        texts[0].text.contains("harbourlight pontoon"),
        "{:?}",
        texts[0].text
    );
    // Ingest read its words for the snippet too.
    let snippet: String = sqlx::query_scalar(
        "SELECT snippet FROM conversations.messages WHERE provider_message_id = 'blank-text-1'",
    )
    .fetch_one(&kit.db)
    .await
    .unwrap();
    assert!(snippet.contains("harbourlight pontoon"), "{snippet:?}");
}
