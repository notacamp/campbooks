//! What a member told Scout about people, over HTTP: kept, changed and
//! forgotten by its member alone.

use axum::http::StatusCode;
use campbooks_kernel::tenancy::{UserId, WorkspaceId};
use serde_json::{Value, json};
use sqlx::PgPool;

use crate::{
    events::MemberLeft,
    model::InboxRole,
    testkit::{ALL_SCOPES, Kit},
};

fn notes(workspace: WorkspaceId) -> String {
    format!("/api/v1/w/{workspace}/person-notes")
}

fn note(workspace: WorkspaceId, id: &str) -> String {
    format!("/api/v1/w/{workspace}/person-notes/{id}")
}

/// The notes `token` lists.
async fn listed(kit: &Kit, token: &str, workspace: WorkspaceId) -> Vec<Value> {
    let response = kit.get(token, &notes(workspace)).await;
    assert_eq!(response.status, StatusCode::OK, "{:?}", response.json);
    response.json["notes"].as_array().unwrap().clone()
}

/// Keep `text` about `address` as `token`; the note as kept.
async fn keep(kit: &Kit, token: &str, workspace: WorkspaceId, address: &str, text: &str) -> Value {
    let response = kit
        .put(
            token,
            &notes(workspace),
            json!({"address": address, "note": text}),
        )
        .await;
    assert_eq!(response.status, StatusCode::OK, "{:?}", response.json);
    response.json
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_member_keeps_changes_and_forgets_a_note(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (sam, _) = kit.member("sam", workspace);

    let kept = keep(
        &kit,
        &sam,
        workspace,
        "  Marta@Clientes.Example ",
        "  My landlord.\u{202E}\r\nFormal,\u{200B} in Portuguese.\t ",
    )
    .await;
    assert_eq!(kept["address"], "marta@clientes.example");
    assert_eq!(kept["note"], "My landlord.\nFormal, in Portuguese.");
    assert!(kept.get("name").is_none(), "no mail gave a name");
    let id = kept["id"].as_str().unwrap().to_owned();

    // The same address again changes the same note.
    let changed = keep(
        &kit,
        &sam,
        workspace,
        "marta@clientes.example",
        "Tu, in Portuguese.",
    )
    .await;
    assert_eq!(changed["id"], id.as_str());
    assert_eq!(changed["note"], "Tu, in Portuguese.");
    keep(&kit, &sam, workspace, "rui@obras.example", "My builder.").await;
    let list = listed(&kit, &sam, workspace).await;
    let addresses: Vec<&str> = list
        .iter()
        .map(|n| n["address"].as_str().unwrap())
        .collect();
    // The newest change first.
    assert_eq!(addresses, ["rui@obras.example", "marta@clientes.example"]);

    let response = kit.delete(&sam, &note(workspace, &id)).await;
    assert_eq!(response.status, StatusCode::NO_CONTENT);
    assert_eq!(listed(&kit, &sam, workspace).await.len(), 1);
    // Forgotten once: 404 the second time.
    let response = kit.delete(&sam, &note(workspace, &id)).await;
    assert_eq!(response.status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_note_is_its_members_alone(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (sam, _) = kit.member("sam", workspace);
    let (ana, _) = kit.member("ana", workspace);
    let mine = keep(
        &kit,
        &sam,
        workspace,
        "marta@clientes.example",
        "My landlord.",
    )
    .await;
    let id = mine["id"].as_str().unwrap();

    // Ana, in the same workspace, neither lists it nor deletes it.
    assert!(listed(&kit, &ana, workspace).await.is_empty());
    let response = kit.delete(&ana, &note(workspace, id)).await;
    assert_eq!(response.status, StatusCode::NOT_FOUND);
    // Her own note about Marta is hers: Sam's stays as he wrote it.
    let hers = keep(&kit, &ana, workspace, "marta@clientes.example", "A client.").await;
    assert_ne!(hers["id"], mine["id"]);
    let list = listed(&kit, &sam, workspace).await;
    assert_eq!(list.len(), 1);
    assert_eq!(list[0]["note"], "My landlord.");
    assert_eq!(listed(&kit, &ana, workspace).await[0]["note"], "A client.");

    // A member of another workspace: 404, nothing deleted.
    let elsewhere = WorkspaceId::new();
    let (bo, _) = kit.member("bo", elsewhere);
    let response = kit.delete(&bo, &note(workspace, id)).await;
    assert_eq!(response.status, StatusCode::NOT_FOUND);
    let response = kit.delete(&bo, &note(elsewhere, id)).await;
    assert_eq!(response.status, StatusCode::NOT_FOUND);
    assert_eq!(listed(&kit, &sam, workspace).await.len(), 1);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_app_acting_for_nobody_has_no_notes(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (sam, _) = kit.member("sam", workspace);
    let mine = keep(
        &kit,
        &sam,
        workspace,
        "marta@clientes.example",
        "My landlord.",
    )
    .await;
    let app = kit.client(workspace, ALL_SCOPES);
    let response = kit.get(&app, &notes(workspace)).await;
    assert_eq!(response.status, StatusCode::FORBIDDEN);
    assert_eq!(response.json["code"], "person_notes_need_member");
    let response = kit
        .put(
            &app,
            &notes(workspace),
            json!({"address": "marta@clientes.example", "note": "x"}),
        )
        .await;
    assert_eq!(response.status, StatusCode::FORBIDDEN);
    let response = kit
        .delete(&app, &note(workspace, mine["id"].as_str().unwrap()))
        .await;
    assert_eq!(response.status, StatusCode::NOT_FOUND);
    assert_eq!(listed(&kit, &sam, workspace).await.len(), 1);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_note_needs_a_plain_address_and_some_words(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (sam, _) = kit.member("sam", workspace);
    let long_address = format!("{}@clientes.example", "m".repeat(320));
    let long_note = "é".repeat(501);
    for (address, text) in [
        ("marta", "My landlord."),
        ("a@b@clientes.example", "My landlord."),
        ("marta @clientes.example", "My landlord."),
        ("\"marta\"@clientes.example", "My landlord."),
        (long_address.as_str(), "My landlord."),
        ("marta@clientes.example", ""),
        ("marta@clientes.example", "  \n "),
        ("marta@clientes.example", "\u{200B}\u{202E}"),
        ("marta@clientes.example", long_note.as_str()),
    ] {
        let response = kit
            .put(
                &sam,
                &notes(workspace),
                json!({"address": address, "note": text}),
            )
            .await;
        assert_eq!(response.status, StatusCode::BAD_REQUEST, "{address:?}");
        assert_eq!(response.json["code"], "person_note_invalid");
    }
    // Unknown fields are refused, so nothing is kept by mistake.
    let response = kit
        .put(
            &sam,
            &notes(workspace),
            json!({"address": "marta@clientes.example", "note": "x", "user_id": UserId::new()}),
        )
        .await;
    assert!(response.status.is_client_error());
    // 500 characters is enough.
    let full = keep(
        &kit,
        &sam,
        workspace,
        "marta@clientes.example",
        &"é".repeat(500),
    )
    .await;
    assert_eq!(full["note"].as_str().unwrap().chars().count(), 500);
    assert_eq!(listed(&kit, &sam, workspace).await.len(), 1);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn five_hundred_notes_at_most(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (sam, user) = kit.member("sam", workspace);
    sqlx::query(
        "INSERT INTO conversations.person_notes (id, workspace_id, user_id, address, note)
         SELECT gen_random_uuid(), $1, $2, 'person' || n || '@clientes.example', 'A client.'
           FROM generate_series(1, 500) n",
    )
    .bind(workspace.as_uuid())
    .bind(user.as_uuid())
    .execute(&kit.db)
    .await
    .unwrap();
    let response = kit
        .put(
            &sam,
            &notes(workspace),
            json!({"address": "marta@clientes.example", "note": "My landlord."}),
        )
        .await;
    assert_eq!(response.status, StatusCode::CONFLICT);
    assert_eq!(response.json["code"], "person_notes_full");
    // One already kept may still change.
    keep(
        &kit,
        &sam,
        workspace,
        "person7@clientes.example",
        "My landlord.",
    )
    .await;
    assert_eq!(listed(&kit, &sam, workspace).await.len(), 500);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_name_comes_from_mail_the_member_reads(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, inbox, sam) = kit.sams_inbox().await;
    kit.ingest_value(
        workspace,
        inbox,
        &json!({"changes": [{"op": "upsert", "message": {
            "provider_message_id": "marta-1",
            "thread_key": "thr-marta",
            "from": {"name": "Marta Ferreira", "address": "marta@clientes.example"},
            "to": [{"name": "Sam Rivera", "address": "sam@example.com"}],
            "subject": "Obra",
            "sent_at": "2026-10-12T09:00:00Z",
            "direction": "inbound",
            "text": "Podemos falar quinta?",
            "mailbox": {"in_inbox": true},
            "read": true, "starred": false
        }}]}),
    )
    .await;
    let kept = keep(
        &kit,
        &sam,
        workspace,
        "marta@clientes.example",
        "My landlord.",
    )
    .await;
    assert_eq!(kept["name"], "Marta Ferreira");
    assert_eq!(
        listed(&kit, &sam, workspace).await[0]["name"],
        "Marta Ferreira"
    );

    // Ana reads no inbox: her note shows the address alone.
    let (ana, ana_id) = kit.member("ana", workspace);
    let hers = keep(&kit, &ana, workspace, "marta@clientes.example", "A client.").await;
    assert!(hers.get("name").is_none());
    // Once she reads the inbox, the name is hers to see too.
    kit.add_to_inbox(inbox, ana_id, InboxRole::Reader).await;
    assert_eq!(
        listed(&kit, &ana, workspace).await[0]["name"],
        "Marta Ferreira"
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_departing_member_s_notes_go(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (sam, _) = kit.member("sam", workspace);
    let (ana, ana_id) = kit.member("ana", workspace);
    keep(
        &kit,
        &sam,
        workspace,
        "marta@clientes.example",
        "My landlord.",
    )
    .await;
    keep(&kit, &ana, workspace, "marta@clientes.example", "A client.").await;

    let left = MemberLeft {
        workspace: workspace.to_string(),
        user: ana_id,
    };
    let mut tx = kit.state.db.begin().await.unwrap();
    campbooks_kernel::db::set_workspace(&mut tx, &workspace.to_string())
        .await
        .unwrap();
    kit.state.events.publish(&mut tx, &left).await.unwrap();
    tx.commit().await.unwrap();
    kit.run_jobs().await;

    let rows: Vec<String> = sqlx::query_scalar("SELECT note FROM conversations.person_notes")
        .fetch_all(&kit.db)
        .await
        .unwrap();
    assert_eq!(rows, ["My landlord."]);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_note_never_reaches_the_event_log_or_the_queue(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (sam, _) = kit.member("sam", workspace);
    let kept = keep(
        &kit,
        &sam,
        workspace,
        "marta@clientes.example",
        "My landlord.",
    )
    .await;
    let response = kit
        .delete(&sam, &note(workspace, kept["id"].as_str().unwrap()))
        .await;
    assert_eq!(response.status, StatusCode::NO_CONTENT);
    for table in ["kernel.events", "kernel.jobs"] {
        let rows: Vec<String> = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "SELECT row_to_json(t)::text FROM {table} t"
        )))
        .fetch_all(&kit.db)
        .await
        .unwrap();
        for row in rows {
            assert!(!row.contains("landlord"), "{table}: {row}");
            assert!(!row.contains("marta@"), "{table}: {row}");
        }
    }
}
