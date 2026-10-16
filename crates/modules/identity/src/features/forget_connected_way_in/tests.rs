//! A way in that came only from connecting a mailbox goes with its last
//! mailbox; every other way in stays.

use axum::http::{Method, StatusCode};
use campbooks_kernel::{db::BeginFor as _, tenancy::WorkspaceId, testing::TestUser};
use chrono::Utc;
use serde_json::{Value, json};
use sqlx::PgPool;

use crate::{domain::mailbox_links, events::ConnectionRemoved, testkit::Kit, upstream::Provider};

const BACK: &str = "/w/ws";

fn user(subject: &str) -> TestUser {
    TestUser {
        subject: subject.to_owned(),
        name: "Sam Rivera".to_owned(),
        email: format!("{subject}@example.com"),
    }
}

/// Connect Google account `subject` as `access` in `workspace` (`sign_in`:
/// also as a way in); the connection id.
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

/// What the connectors module publishes when the mailbox of Google account
/// `subject` in `workspace` goes.
async fn removed(kit: &Kit, workspace: WorkspaceId, subject: &str) {
    let mut tx = kit.state.db.begin_for(workspace).await.unwrap();
    kit.state
        .events
        .publish(
            &mut tx,
            &ConnectionRemoved {
                workspace: workspace.to_string(),
                connection: format!("connection-of-{subject}"),
                provider: Some("google".to_owned()),
                account_subject: Some(subject.to_owned()),
            },
        )
        .await
        .unwrap();
    tx.commit().await.unwrap();
    kit.run_jobs().await;
}

fn addresses(ways: &Value) -> Vec<String> {
    ways["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|w| w["email"].as_str().unwrap_or_default().to_owned())
        .collect()
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_way_in_a_connect_brought_goes_with_its_last_mailbox(db: PgPool) {
    let kit = Kit::new(db).await;
    let access = kit.signed_up().await;
    let (one, two) = (WorkspaceId::new(), WorkspaceId::new());
    let first = connect(&kit, &access, one, "work-sam", true).await;
    connect(&kit, &access, two, "work-sam", true).await;
    let ways = kit.ways_in(&access).await;
    assert_eq!(ways["items"].as_array().unwrap().len(), 2);
    // Ways to sign in says how many mailboxes it came with, and which.
    let work = &ways["items"][1];
    assert_eq!(work["mailboxes"], 2);
    assert!(
        work["connections"]
            .as_array()
            .unwrap()
            .contains(&json!(first))
    );
    assert_eq!(ways["items"][0]["mailboxes"], 0);

    // Its mailbox is still connected in the other workspace: it stays.
    removed(&kit, one, "work-sam").await;
    assert!(addresses(&kit.ways_in(&access).await).contains(&"work-sam@example.com".to_owned()));

    // Its last mailbox goes: so does the way in, announced and audited.
    removed(&kit, two, "work-sam").await;
    let left = addresses(&kit.ways_in(&access).await);
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
    removed(&kit, two, "work-sam").await;
    assert_eq!(
        kit.ways_in(&access).await["items"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn two_of_its_mailboxes_removed_at_once_still_remove_it(db: PgPool) {
    let kit = Kit::new(db).await;
    let access = kit.signed_up().await;
    let (one, two) = (WorkspaceId::new(), WorkspaceId::new());
    connect(&kit, &access, one, "work-sam", true).await;
    connect(&kit, &access, two, "work-sam", true).await;

    // Both removals in their own transactions at the same time: the account
    // lock orders them, so the second sees the first's row gone.
    let forget = |workspace: WorkspaceId| {
        let db = kit.state.db.clone();
        async move {
            let mut tx = db.begin().await.unwrap();
            let gone = mailbox_links::forget(
                &mut tx,
                *workspace.as_uuid(),
                "google",
                "work-sam",
                Utc::now(),
            )
            .await
            .unwrap();
            tx.commit().await.unwrap();
            gone
        }
    };
    let (a, b) = tokio::join!(forget(one), forget(two));
    assert_eq!(
        usize::from(a.is_some()) + usize::from(b.is_some()),
        1,
        "exactly one of them removes it"
    );
    assert!(!addresses(&kit.ways_in(&access).await).contains(&"work-sam@example.com".to_owned()));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn whoever_signed_in_with_it_is_signed_out(db: PgPool) {
    let kit = Kit::new(db).await;
    let access = kit.signed_up().await;
    let ws = WorkspaceId::new();
    connect(&kit, &access, ws, "work-sam", true).await;
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

    removed(&kit, ws, "work-sam").await;
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
    let signed_up_with = addresses(&kit.ways_in(&access).await);
    let own = signed_up_with[0].replace("@example.com", "");

    // The account signed up with, connected with the box unticked: not a
    // connect way in. A shared mailbox: never a way in.
    connect(&kit, &access, ws, &own, false).await;
    connect(&kit, &access, ws, "support", false).await;
    removed(&kit, ws, &own).await;
    removed(&kit, ws, "support").await;
    assert_eq!(addresses(&kit.ways_in(&access).await), signed_up_with);

    // A mailbox nobody recorded (before this record existed): nothing changes.
    removed(&kit, ws, "from-before").await;
    assert_eq!(addresses(&kit.ways_in(&access).await), signed_up_with);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_last_way_in_stays(db: PgPool) {
    let kit = Kit::new(db).await;
    let access = kit.signed_up().await;
    let ws = WorkspaceId::new();
    connect(&kit, &access, ws, "work-sam", true).await;
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
    removed(&kit, ws, "work-sam").await;
    assert_eq!(
        addresses(&kit.ways_in(&access).await),
        ["work-sam@example.com"]
    );
}
