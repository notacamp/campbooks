//! A way in that came only from connecting a mailbox goes with its last
//! mailbox; every other way in stays.

use axum::http::{Method, StatusCode};
use campbooks_kernel::{db::BeginFor as _, tenancy::WorkspaceId, testing::TestUser};
use chrono::Utc;
use serde_json::{Value, json};
use sqlx::PgPool;

use crate::{
    domain::mailbox_links,
    events::ConnectionRemoved,
    testkit::Kit,
    tracking_tests::{account_of, no_address, tracked},
    upstream::Provider,
};

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

/// What the connectors module does when connection `connection` (of Google
/// account `subject`) in `workspace` goes: it removes the connection and
/// publishes `connectors.connection_removed`.
async fn removed_connection(kit: &Kit, workspace: WorkspaceId, connection: &str, subject: &str) {
    kit.intake.drop_connection(connection);
    announce_removed(kit, workspace, connection, subject).await;
}

/// `connectors.connection_removed` for connection `connection` (of Google
/// account `subject`) in `workspace`, whether or not it went.
async fn announce_removed(kit: &Kit, workspace: WorkspaceId, connection: &str, subject: &str) {
    let mut tx = kit.state.db.begin_for(workspace).await.unwrap();
    kit.state
        .events
        .publish(
            &mut tx,
            &ConnectionRemoved {
                workspace: workspace.to_string(),
                connection: connection.to_owned(),
                provider: Some("google".to_owned()),
                account_subject: Some(subject.to_owned()),
            },
        )
        .await
        .unwrap();
    tx.commit().await.unwrap();
    kit.run_jobs().await;
}

/// The connection identity recorded for the mailbox of Google account
/// `subject` in `workspace` (or an unknown one when none is recorded).
async fn recorded(kit: &Kit, workspace: WorkspaceId, subject: &str) -> String {
    sqlx::query_scalar::<_, Option<String>>(
        "SELECT l.connection_id FROM identity.mailbox_links l
           JOIN identity.provider_identities p ON p.id = l.provider_identity_id
          WHERE l.in_workspace_id = $1 AND p.subject = $2",
    )
    .bind(workspace.as_uuid())
    .bind(subject)
    .fetch_optional(&kit.state.db)
    .await
    .unwrap()
    .flatten()
    .unwrap_or_else(|| "never-recorded".to_owned())
}

/// The connectors module removes the connection it holds for Google account
/// `subject` in `workspace`.
async fn removed(kit: &Kit, workspace: WorkspaceId, subject: &str) {
    let connection = recorded(kit, workspace, subject).await;
    removed_connection(kit, workspace, &connection, subject).await;
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
    .fetch_one(&kit.kernel)
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
    // Tracked once, in the workspace of the last connection, by the handler
    // (ADR 0020): ids, the provider and a count, no address.
    let tracked = tracked(&kit, "identity.connected_way_in_removed").await;
    assert_eq!(tracked.len(), 1);
    let event = &tracked[0];
    assert_eq!(event.version, 1);
    assert_eq!(event.workspace_id, Some(*two.as_uuid()));
    assert_eq!(
        event.envelope["actor"],
        json!({"type": "system", "id": "identity.forget_connected_way_in"})
    );
    assert_eq!(event.body["account_id"], account_of(&access));
    assert_eq!(event.body["provider"], "google");
    assert_eq!(event.body["sessions_ended"], 0);
    assert!(event.body["way_in_id"].is_string());
    no_address(event);
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
    let (first, second) = (
        recorded(&kit, one, "work-sam").await,
        recorded(&kit, two, "work-sam").await,
    );
    let forget = |workspace: WorkspaceId, connection: String| {
        let db = kit.state.db.clone();
        async move {
            let mut tx = db.begin().await.unwrap();
            let gone = mailbox_links::forget(
                &mut tx,
                *workspace.as_uuid(),
                &connection,
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
    let (a, b) = tokio::join!(forget(one, first), forget(two, second));
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
    // This session signed in with the way in it removed: it is signed out,
    // its token stopped at once (campbooks-cloud#143). Signed in again with
    // the one left, that one stays.
    let stopped = kit
        .api(Method::GET, "/api/v1/identity/ways-in", &access)
        .await;
    assert_eq!(stopped.status, StatusCode::UNAUTHORIZED, "{stopped:?}");
    kit.new_browser();
    kit.google.set_user(user("work-sam"));
    let (url, verifier) = Kit::web_authorize();
    let request = kit.get(&url).await.location_query()["request"].clone();
    let back = kit.round_trip(Some(&request)).await;
    let tokens = kit.redeem(&back.location_query()["code"], &verifier).await;
    assert_eq!(tokens.status, StatusCode::OK, "{tokens:?}");
    let fresh = tokens.json["access_token"].as_str().unwrap().to_owned();
    assert_eq!(
        addresses(&kit.ways_in(&fresh).await),
        ["work-sam@example.com"]
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn another_members_disconnect_never_removes_your_way_in(db: PgPool) {
    let kit = Kit::new(db).await;
    let (theirs, mine) = (WorkspaceId::new(), WorkspaceId::new());
    // B connects the shared support@ mailbox in their workspace, not as a
    // way in.
    kit.google.set_user(user("bea"));
    let bea = kit.signed_up().await;
    let bea_connection = connect(&kit, &bea, theirs, "support", false).await;
    // A made support@ a way in by connecting it in another workspace, then
    // connects it in B's: the connectors module refuses (it is B's there).
    kit.new_browser();
    kit.google.set_user(user("ana"));
    let ana = kit.signed_up().await;
    connect(&kit, &ana, mine, "support", true).await;
    kit.intake.refuse_next();
    kit.google.set_user(user("support"));
    let started = kit
        .start_link(
            &ana,
            &json!({ "purpose": "connect", "provider": "google", "workspace_id": theirs,
                     "sign_in": true, "return_to": BACK }),
        )
        .await;
    let to_google = kit.get(started.json["url"].as_str().unwrap()).await;
    let back = kit.via(Provider::Google, &to_google.location()).await;
    assert!(back.location().contains("link_error="), "{back:?}");
    // The failed hand-over took back its record in B's workspace; the way
    // in (made earlier) stays.
    assert_eq!(recorded(&kit, theirs, "support").await, "never-recorded");
    assert!(addresses(&kit.ways_in(&ana).await).contains(&"support@example.com".to_owned()));

    // B disconnects: A's way in is untouched.
    removed_connection(&kit, theirs, &bea_connection, "support").await;
    assert!(addresses(&kit.ways_in(&ana).await).contains(&"support@example.com".to_owned()));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_refused_connect_takes_back_the_way_in_it_made(db: PgPool) {
    let kit = Kit::new(db).await;
    let access = kit.signed_up().await;
    let ws = WorkspaceId::new();
    kit.intake.refuse_next();
    kit.google.set_user(user("support"));
    let started = kit
        .start_link(
            &access,
            &json!({ "purpose": "connect", "provider": "google", "workspace_id": ws,
                     "sign_in": true, "return_to": BACK }),
        )
        .await;
    let to_google = kit.get(started.json["url"].as_str().unwrap()).await;
    let back = kit.via(Provider::Google, &to_google.location()).await;
    assert!(back.location().contains("link_error="), "{back:?}");
    // Nothing connected: no sign-in with support@ is left behind, nor a record.
    assert!(!addresses(&kit.ways_in(&access).await).contains(&"support@example.com".to_owned()));
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM identity.mailbox_links")
        .fetch_one(&kit.state.db)
        .await
        .unwrap();
    assert_eq!(rows, 0);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_record_naming_no_connection_is_never_acted_on(db: PgPool) {
    let kit = Kit::new(db).await;
    let access = kit.signed_up().await;
    let ws = WorkspaceId::new();
    connect(&kit, &access, ws, "work-sam", true).await;
    // As if naming the connection had failed (or the hand-over were still
    // in flight): the row names none.
    sqlx::query("UPDATE identity.mailbox_links SET connection_id = NULL")
        .execute(&kit.state.db)
        .await
        .unwrap();
    removed_connection(&kit, ws, "someone-elses-connection", "work-sam").await;
    assert!(addresses(&kit.ways_in(&access).await).contains(&"work-sam@example.com".to_owned()));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_failed_hand_over_leaves_no_record_and_no_way_in(db: PgPool) {
    let kit = Kit::new(db).await;
    let access = kit.signed_up().await;
    let ws = WorkspaceId::new();
    kit.google.set_user(user("work-sam"));
    let started = kit
        .start_link(
            &access,
            &json!({ "purpose": "connect", "provider": "google", "workspace_id": ws,
                     "sign_in": true, "return_to": BACK }),
        )
        .await;
    let to_google = kit.get(started.json["url"].as_str().unwrap()).await;
    // The person unticks the mail permission: nothing is connected.
    let declined = to_google.location().replace(
        &url::form_urlencoded::byte_serialize(crate::testkit::MAIL_SCOPE.as_bytes())
            .collect::<String>(),
        "",
    );
    let back = kit.via(Provider::Google, &declined).await;
    assert_eq!(
        back.location(),
        format!("{BACK}?link_error=consent_incomplete")
    );
    // The way in this connect made goes with it.
    assert!(!addresses(&kit.ways_in(&access).await).contains(&"work-sam@example.com".to_owned()));
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM identity.mailbox_links")
        .fetch_one(&kit.state.db)
        .await
        .unwrap();
    assert_eq!(rows, 0);
}

/// The event alone is not believed (campbooks-cloud#211): a
/// `connectors.connection_removed` written in the workspace for a
/// connection the connectors module still holds leaves the way in, its
/// record and its sessions alone.
#[sqlx::test(migrations = "../../../migrations")]
async fn a_forged_removal_of_a_live_connection_changes_nothing(db: PgPool) {
    let kit = Kit::new(db).await;
    let access = kit.signed_up().await;
    let ws = WorkspaceId::new();
    let connection = connect(&kit, &access, ws, "work-sam", true).await;
    let before = kit.ways_in(&access).await;
    assert!(addresses(&before).contains(&"work-sam@example.com".to_owned()));

    // Still connected, yet announced as removed.
    announce_removed(&kit, ws, &connection, "work-sam").await;
    assert_eq!(addresses(&kit.ways_in(&access).await), addresses(&before));
    assert_eq!(recorded(&kit, ws, "work-sam").await, connection);
    let removals: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM kernel.events WHERE name = 'identity.way_in_removed'",
    )
    .fetch_one(&kit.kernel)
    .await
    .unwrap();
    assert_eq!(removals, 0);

    // The connectors module cannot answer: nothing is decided on a guess.
    kit.intake.drop_connection(&connection);
    kit.intake.cannot_answer();
    announce_removed(&kit, ws, &connection, "work-sam").await;
    assert!(addresses(&kit.ways_in(&access).await).contains(&"work-sam@example.com".to_owned()));
    assert_eq!(recorded(&kit, ws, "work-sam").await, connection);
}
