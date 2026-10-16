//! A mailbox record the round trip never named is named by the connection
//! it was granted for; one nothing ever names is swept, with the way in it
//! leaves behind.

use axum::http::StatusCode;
use campbooks_kernel::{
    db::BeginFor as _,
    jobs::{self, EnqueueOptions},
    tenancy::WorkspaceId,
    testing::TestUser,
};
use serde_json::{Value, json};
use sqlx::PgPool;

use crate::{Prune, events::ConnectionAdded, testkit::Kit, upstream::Provider};

const BACK: &str = "/w/ws";

fn user(subject: &str) -> TestUser {
    TestUser {
        subject: subject.to_owned(),
        name: "Sam Rivera".to_owned(),
        email: format!("{subject}@example.com"),
    }
}

/// Connect Google account `subject` as `access` in `workspace`, as a way in;
/// then forget the connection on its record, as when the round trip dies
/// before naming it. The connection id.
async fn connected_unnamed(
    kit: &Kit,
    access: &str,
    workspace: WorkspaceId,
    subject: &str,
) -> String {
    kit.google.set_user(user(subject));
    let started = kit
        .start_link(
            access,
            &json!({ "purpose": "connect", "provider": "google", "workspace_id": workspace,
                     "sign_in": true, "return_to": BACK }),
        )
        .await;
    assert_eq!(started.status, StatusCode::CREATED, "{started:?}");
    let to_google = kit.get(started.json["url"].as_str().unwrap()).await;
    let back = kit.via(Provider::Google, &to_google.location()).await;
    let connection = back
        .location()
        .strip_prefix(&format!("{BACK}?connected="))
        .unwrap_or_else(|| panic!("not connected: {}", back.location()))
        .to_owned();
    sqlx::query(
        "UPDATE identity.mailbox_links SET connection_id = NULL WHERE in_workspace_id = $1",
    )
    .bind(workspace.as_uuid())
    .execute(&kit.state.db)
    .await
    .unwrap();
    connection
}

/// The account behind `access`.
fn account_of(access: &str) -> String {
    Kit::decode(access).1["sub"].as_str().unwrap().to_owned()
}

/// What the connectors module publishes when it adopts a grant.
async fn added(kit: &Kit, workspace: WorkspaceId, connection: &str, subject: &str, grantor: &str) {
    let mut tx = kit.state.db.begin_for(workspace).await.unwrap();
    kit.state
        .events
        .publish(
            &mut tx,
            &ConnectionAdded {
                workspace: workspace.to_string(),
                connection: connection.to_owned(),
                provider: "google".to_owned(),
                account_subject: Some(subject.to_owned()),
                grantor_subject: Some(grantor.to_owned()),
            },
        )
        .await
        .unwrap();
    tx.commit().await.unwrap();
    kit.run_jobs().await;
}

/// The records: (workspace, connection) per row.
async fn records(kit: &Kit) -> Vec<(uuid::Uuid, Option<String>)> {
    sqlx::query_as("SELECT in_workspace_id, connection_id FROM identity.mailbox_links ORDER BY 1")
        .fetch_all(&kit.state.db)
        .await
        .unwrap()
}

fn addresses(ways: &Value) -> Vec<String> {
    ways["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|w| w["email"].as_str().unwrap_or_default().to_owned())
        .collect()
}

/// Run the daily prune, as if `hours` had passed since every record was made.
async fn prune_after(kit: &Kit, hours: i64) {
    sqlx::query(
        "UPDATE identity.mailbox_links SET created_at = created_at - make_interval(hours => $1)",
    )
    .bind(i32::try_from(hours).unwrap())
    .execute(&kit.state.db)
    .await
    .unwrap();
    let mut tx = kit.state.db.begin().await.unwrap();
    jobs::enqueue(&mut tx, &Prune, EnqueueOptions::default())
        .await
        .unwrap();
    tx.commit().await.unwrap();
    kit.run_jobs().await;
}

async fn removals(kit: &Kit) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM kernel.events WHERE name = 'identity.way_in_removed'")
        .fetch_one(&kit.state.db)
        .await
        .unwrap()
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_unnamed_record_is_named_by_its_connection(db: PgPool) {
    let kit = Kit::new(db).await;
    let access = kit.signed_up().await;
    let ws = WorkspaceId::new();
    let connection = connected_unnamed(&kit, &access, ws, "work-sam").await;
    assert_eq!(records(&kit).await, vec![(*ws.as_uuid(), None)]);

    added(&kit, ws, &connection, "work-sam", &account_of(&access)).await;
    assert_eq!(
        records(&kit).await,
        vec![(*ws.as_uuid(), Some(connection.clone()))]
    );
    // A replay names the same connection again: nothing changes.
    added(&kit, ws, &connection, "work-sam", &account_of(&access)).await;
    assert_eq!(records(&kit).await, vec![(*ws.as_uuid(), Some(connection))]);

    // Named, it is never swept: the way in stays.
    prune_after(&kit, 48).await;
    assert!(addresses(&kit.ways_in(&access).await).contains(&"work-sam@example.com".to_owned()));
    assert_eq!(removals(&kit).await, 0);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_connection_granted_by_someone_else_names_nothing(db: PgPool) {
    let kit = Kit::new(db).await;
    let access = kit.signed_up().await;
    let ws = WorkspaceId::new();
    let connection = connected_unnamed(&kit, &access, ws, "work-sam").await;
    kit.new_browser();
    kit.google.set_user(user("bea"));
    let other = kit.signed_up().await;

    // The same provider account, handed over for another account: not this
    // account's way in to name.
    added(&kit, ws, &connection, "work-sam", &account_of(&other)).await;
    assert_eq!(records(&kit).await, vec![(*ws.as_uuid(), None)]);
    // Nor does a connection with no grantor, or a way in nobody tracks.
    added(&kit, ws, "c-other", "bea", &account_of(&other)).await;
    assert_eq!(records(&kit).await, vec![(*ws.as_uuid(), None)]);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_record_never_named_is_swept_with_the_way_in_it_left(db: PgPool) {
    let kit = Kit::new(db).await;
    let access = kit.signed_up().await;
    let ws = WorkspaceId::new();
    connected_unnamed(&kit, &access, ws, "work-sam").await;

    // Within the grace period, it may still be named: it stays.
    prune_after(&kit, 2).await;
    assert_eq!(records(&kit).await.len(), 1);
    assert!(addresses(&kit.ways_in(&access).await).contains(&"work-sam@example.com".to_owned()));

    // A day on, nothing named it: the record goes, and the way in it alone
    // brought goes with it, announced.
    prune_after(&kit, 24).await;
    assert!(records(&kit).await.is_empty());
    let left = addresses(&kit.ways_in(&access).await);
    assert!(
        !left.contains(&"work-sam@example.com".to_owned()),
        "{left:?}"
    );
    assert_eq!(left.len(), 1);
    assert_eq!(removals(&kit).await, 1);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_sweep_never_removes_an_accounts_last_way_in(db: PgPool) {
    let kit = Kit::new(db).await;
    let access = kit.signed_up().await;
    let ws = WorkspaceId::new();
    connected_unnamed(&kit, &access, ws, "work-sam").await;
    // The way in the account signed up with goes; the connected one is left.
    let ways = kit.ways_in(&access).await;
    let first = ways["items"][0]["id"].as_str().unwrap().to_owned();
    sqlx::query("DELETE FROM identity.provider_identities WHERE id = $1::uuid")
        .bind(&first)
        .execute(&kit.state.db)
        .await
        .unwrap();

    prune_after(&kit, 48).await;
    assert!(records(&kit).await.is_empty());
    assert_eq!(
        addresses(&kit.ways_in(&access).await),
        vec!["work-sam@example.com".to_owned()]
    );
    assert_eq!(removals(&kit).await, 0);
}
