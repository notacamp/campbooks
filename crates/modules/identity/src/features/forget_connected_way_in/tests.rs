//! A way in that came only from connecting a mailbox goes with its last
//! connection; every other way in stays.

use axum::http::{Method, StatusCode};
use campbooks_kernel::{db::BeginFor as _, tenancy::WorkspaceId, testing::TestUser};
use serde_json::{Value, json};
use sqlx::PgPool;

use crate::{events::ConnectionRemoved, testkit::Kit, upstream::Provider};

const BACK: &str = "/w/ws";

fn user(subject: &str) -> TestUser {
    TestUser {
        subject: subject.to_owned(),
        name: "Sam Rivera".to_owned(),
        email: format!("{subject}@example.com"),
    }
}

/// Connect Google account `subject` as `access` in `workspace`
/// (`sign_in`: also as a way in); the connection id.
async fn connect(
    kit: &Kit,
    access: &str,
    workspace: WorkspaceId,
    subject: &str,
    sign_in: bool,
) -> String {
    kit.google.set_user(user(subject));
    let started = kit
        .start_link(
            access,
            &json!({ "purpose": "connect", "provider": "google", "workspace_id": workspace,
                     "sign_in": sign_in, "return_to": BACK }),
        )
        .await;
    assert_eq!(started.status, StatusCode::CREATED, "{started:?}");
    let to_google = kit.get(started.json["url"].as_str().unwrap()).await;
    let back = kit.via(Provider::Google, &to_google.location()).await;
    back.location()
        .strip_prefix(&format!("{BACK}?connected="))
        .unwrap_or_else(|| panic!("not connected: {}", back.location()))
        .to_owned()
}

/// What the connectors module publishes when `connection` goes.
async fn removed(kit: &Kit, workspace: WorkspaceId, connection: &str) {
    let mut tx = kit.state.db.begin_for(workspace).await.unwrap();
    kit.state
        .events
        .publish(
            &mut tx,
            &ConnectionRemoved {
                workspace: workspace.to_string(),
                connection: connection.to_owned(),
            },
        )
        .await
        .unwrap();
    tx.commit().await.unwrap();
    kit.run_jobs().await;
}

fn providers(ways: &Value) -> Vec<String> {
    ways["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|w| w["email"].as_str().unwrap_or_default().to_owned())
        .collect()
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_way_in_a_connect_brought_goes_with_its_last_connection(db: PgPool) {
    let kit = Kit::new(db).await;
    let access = kit.signed_up().await;
    let (one, two) = (WorkspaceId::new(), WorkspaceId::new());
    let first = connect(&kit, &access, one, "work-sam", true).await;
    let second = connect(&kit, &access, two, "work-sam", true).await;
    assert_eq!(
        kit.ways_in(&access).await["items"]
            .as_array()
            .unwrap()
            .len(),
        2
    );

    // Another connection still uses it: it stays.
    removed(&kit, one, &first).await;
    assert!(providers(&kit.ways_in(&access).await).contains(&"work-sam@example.com".to_owned()));

    // Its last connection goes: so does the way in, announced and audited.
    removed(&kit, two, &second).await;
    let left = providers(&kit.ways_in(&access).await);
    assert!(
        !left.contains(&"work-sam@example.com".to_owned()),
        "{left:?}"
    );
    assert_eq!(left.len(), 1);
    let events: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM kernel.events WHERE name = 'identity.way_in_removed'",
    )
    .fetch_one(&kit.state.db)
    .await
    .unwrap();
    assert_eq!(events, 1);
    // A replay finds nothing left to do.
    removed(&kit, two, &second).await;
    assert_eq!(
        kit.ways_in(&access).await["items"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn whoever_signed_in_with_it_is_signed_out(db: PgPool) {
    let kit = Kit::new(db).await;
    let access = kit.signed_up().await;
    let ws = WorkspaceId::new();
    let connection = connect(&kit, &access, ws, "work-sam", true).await;
    // Another browser signs in with the connected account.
    kit.new_browser();
    let (url, _) = Kit::web_authorize();
    let request = kit.get(&url).await.location_query()["request"].clone();
    kit.google.set_user(user("work-sam"));
    kit.round_trip_with(Provider::Google, Some(&request)).await;
    let sessions: Vec<uuid::Uuid> = sqlx::query_scalar(
        "SELECT s.id FROM identity.sessions s
           JOIN identity.provider_identities p ON p.id = s.provider_identity_id
          WHERE p.email_at_link = 'work-sam@example.com' AND s.ended_at IS NULL",
    )
    .fetch_all(&kit.state.db)
    .await
    .unwrap();
    assert_eq!(sessions.len(), 1, "signed in with it");

    removed(&kit, ws, &connection).await;
    let live: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM identity.sessions WHERE id = ANY($1) AND ended_at IS NULL",
    )
    .bind(&sessions)
    .fetch_one(&kit.state.db)
    .await
    .unwrap();
    assert_eq!(live, 0);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn other_ways_in_stay(db: PgPool) {
    let kit = Kit::new(db).await;
    let access = kit.signed_up().await;
    let ws = WorkspaceId::new();
    let signed_up_with = providers(&kit.ways_in(&access).await);

    // The account signed up with, connected (unticked or not): not a connect way in.
    let own = connect(
        &kit,
        &access,
        ws,
        &signed_up_with[0].replace("@example.com", ""),
        false,
    )
    .await;
    // A shared mailbox: never a way in.
    let shared = connect(&kit, &access, ws, "support", false).await;
    removed(&kit, ws, &own).await;
    removed(&kit, ws, &shared).await;
    assert_eq!(providers(&kit.ways_in(&access).await), signed_up_with);

    // A connection nobody recorded (before this record existed): nothing changes.
    removed(&kit, ws, "connection-from-before").await;
    assert_eq!(providers(&kit.ways_in(&access).await), signed_up_with);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_last_way_in_stays(db: PgPool) {
    let kit = Kit::new(db).await;
    let access = kit.signed_up().await;
    let ws = WorkspaceId::new();
    let connection = connect(&kit, &access, ws, "work-sam", true).await;
    // The account signed up with is removed, so the connected one is the last.
    let ways = kit.ways_in(&access).await;
    let first = ways["items"][0]["id"].as_str().unwrap().to_owned();
    let gone = kit
        .api(
            Method::DELETE,
            &format!("/api/v1/identity/ways-in/{first}"),
            &access,
        )
        .await;
    assert_eq!(gone.status, StatusCode::NO_CONTENT, "{gone:?}");
    removed(&kit, ws, &connection).await;
    assert_eq!(
        providers(&kit.ways_in(&access).await),
        ["work-sam@example.com"]
    );
}
