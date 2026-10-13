//! Connecting an account: `start_connection` checks what the connectors
//! module owns and delegates the consent to the identity module's front
//! door; the grant comes back through the grant-intake port.

use axum::http::StatusCode;
use campbooks_kernel::{
    blobs::BlobRef,
    tenancy::{Role, WorkspaceId},
};
use sqlx::PgPool;

use crate::testkit::{ACCOUNT_SUB, Kit, assert_contract};

#[sqlx::test(migrations = "../../../migrations")]
async fn starting_a_connection_goes_through_the_front_door(db: PgPool) {
    let kit = Kit::new(db).await;
    let ws = WorkspaceId::new();
    let ana = kit.user("ana");
    kit.join(&ana, ws, Role::Member);
    let started = kit.start(&ana, ws).await;
    assert_eq!(started.status, StatusCode::CREATED, "{:?}", started.json);
    assert_eq!(
        started.json["authorize_url"],
        "/auth/start/google?link=fake-ticket"
    );
    assert!(started.json["expires_at"].is_string());
    assert_eq!(
        kit.door.asked(),
        [(ws, "ana".to_owned(), "google".to_owned(), None)]
    );

    // Re-authorising your own connection names it.
    let connection = kit.connect(&ana, ws).await;
    let id = connection["id"].as_str().unwrap().to_owned();
    let again = kit
        .post(
            &ana,
            &format!("/api/v1/w/{ws}/connections/authorizations"),
            serde_json::json!({"provider": "google", "capabilities": ["messages"], "connection": id}),
        )
        .await;
    assert_eq!(again.status, StatusCode::CREATED);
    assert_eq!(kit.door.asked()[1].3.as_deref(), Some(id.as_str()));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_reconnect_is_for_its_grantor_and_its_account_only(db: PgPool) {
    let kit = Kit::new(db).await;
    let ws = WorkspaceId::new();
    let ana = kit.user("ana");
    kit.join(&ana, ws, Role::Member);
    let bea = kit.user("bea");
    kit.join(&bea, ws, Role::Member);
    let id = kit.connect(&ana, ws).await["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let intake = crate::grant_intake(kit.deps.clone());

    let target = intake
        .reconnect_target(&kit.state, "ana", ws, &id)
        .await
        .unwrap();
    assert_eq!(target.provider, "google");
    assert_eq!(target.address, "sam@example.com");
    for (who, connection) in [("bea", id.as_str()), ("eve", id.as_str()), ("ana", "nope")] {
        assert!(
            matches!(
                intake
                    .reconnect_target(&kit.state, who, ws, connection)
                    .await,
                Err(campbooks_kernel::Error::NotFound)
            ),
            "{who} {connection}"
        );
    }

    // The same account re-authorises it; another account is refused.
    assert_eq!(
        kit.adopt(&ana, ws, ACCOUNT_SUB, Some(id.clone()))
            .await
            .unwrap(),
        id
    );
    let other = kit
        .adopt(&ana, ws, "google-sub-other", Some(id.clone()))
        .await
        .unwrap_err();
    assert_eq!(other.code(), "account_mismatch");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_member_connects_gmail(db: PgPool) {
    let kit = Kit::new(db).await;
    let ws = WorkspaceId::new();
    let ana = kit.user("ana");
    let ana_id = kit.join(&ana, ws, Role::Member);

    let connection = kit.connect(&ana, ws).await;
    assert_eq!(connection["provider"], "google");
    assert_eq!(connection["address"], "sam@example.com");
    assert_eq!(connection["address_verified"], true);
    assert_eq!(connection["display_name"], "Sam Rivera");
    assert_eq!(connection["granted_by"], ana_id.to_string());
    assert_eq!(connection["capabilities"], serde_json::json!(["messages"]));
    assert_eq!(connection["health"], "ok");
    let stream = &connection["streams"][0];
    assert_eq!(stream["capability"], "messages");
    assert_eq!(stream["phase"], "backfill");
    assert!(stream["inbox_id"].is_string());
    assert!(stream["next_sync_at"].is_string());
    // No credential in the answer.
    let text = connection.to_string();
    assert!(
        !text.contains("fake-refresh") && !text.contains("fake-access"),
        "{text}"
    );

    // Credentials are sealed at rest.
    let sealed = sqlx::query!("SELECT refresh_token, access_token FROM connectors.credentials")
        .fetch_one(&kit.db)
        .await
        .unwrap();
    for bytes in [sealed.refresh_token, sealed.access_token.unwrap()] {
        let lossy = String::from_utf8_lossy(&bytes);
        assert!(!lossy.contains("fake-refresh") && !lossy.contains("fake-access"));
    }

    // Announced: the connection, then the capability with its inbox.
    assert_eq!(
        kit.events_of(ws).await,
        [
            "connectors.connection_added",
            "connectors.capability_granted"
        ]
    );
    let granted = kit
        .payloads(ws, "connectors.capability_granted")
        .await
        .remove(0);
    assert_contract("connectors.capability_granted.v1.schema.json", &granted);
    assert_eq!(granted["inbox_id"], stream["inbox_id"]);
    assert_eq!(granted["granted_by"], ana_id.to_string());
    let account: BlobRef = serde_json::from_value(granted["account"].clone()).unwrap();
    let account: serde_json::Value = serde_json::from_slice(&kit.blob(ws, &account).await).unwrap();
    assert_contract("connection-account.v1.schema.json", &account);
    assert_eq!(account["address"], "sam@example.com");
    // The account document is held by the connection.
    let held: i64 = sqlx::query_scalar!(
        r#"SELECT count(*) AS "n!" FROM kernel.blob_refs WHERE owner_type = 'connectors.connection'"#
    )
    .fetch_one(&kit.db)
    .await
    .unwrap();
    assert_eq!(held, 1);
    // The first run is waiting.
    assert_eq!(kit.pending("connectors.sync_stream").await, 1);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn connecting_the_same_account_again_keeps_one_connection(db: PgPool) {
    let kit = Kit::new(db).await;
    let ws = WorkspaceId::new();
    let ana = kit.user("ana");
    kit.join(&ana, ws, Role::Member);
    let first = kit.connect(&ana, ws).await;
    let second = kit.connect(&ana, ws).await;
    assert_eq!(first["id"], second["id"]);
    let rows: Vec<String> =
        sqlx::query_scalar!("SELECT account_subject FROM connectors.connections")
            .fetch_all(&kit.db)
            .await
            .unwrap();
    assert_eq!(rows, [ACCOUNT_SUB]);
    // No second announcement of what it already had.
    assert_eq!(
        kit.payloads(ws, "connectors.capability_granted")
            .await
            .len(),
        1
    );

    // Another member connecting the same account is refused.
    let bea = kit.user("bea");
    kit.join(&bea, ws, Role::Member);
    let taken = kit.adopt(&bea, ws, ACCOUNT_SUB, None).await.unwrap_err();
    assert_eq!(taken.code(), "account_connected_by_another_member");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn refusals(db: PgPool) {
    let kit = Kit::new(db).await;
    let ws = WorkspaceId::new();
    let ana = kit.user("ana");
    kit.join(&ana, ws, Role::Member);
    let uri = format!("/api/v1/w/{ws}/connections/authorizations");

    let calendar = kit
        .post(
            &ana,
            &uri,
            serde_json::json!({"provider": "google", "capabilities": ["calendar"]}),
        )
        .await;
    assert_eq!(calendar.status, StatusCode::BAD_REQUEST);
    assert_eq!(calendar.json["code"], "capability_not_offered");
    let nothing = kit
        .post(
            &ana,
            &uri,
            serde_json::json!({"provider": "google", "capabilities": []}),
        )
        .await;
    assert_eq!(nothing.json["code"], "capabilities_required");

    let client = kit.start(&kit.client(ws), ws).await;
    assert_eq!(client.status, StatusCode::FORBIDDEN);
    assert_eq!(client.json["code"], "connection_needs_user");

    assert!(
        kit.door.asked().is_empty(),
        "no refusal reached the front door"
    );
    let stranger = kit.user("eve");
    assert_eq!(kit.start(&stranger, ws).await.status, StatusCode::NOT_FOUND);
    let anonymous = kit
        .post(
            "",
            &uri,
            serde_json::json!({"provider": "google", "capabilities": ["messages"]}),
        )
        .await;
    assert_eq!(anonymous.status, StatusCode::UNAUTHORIZED);
    let read_only = kit.user_scopes("ana", "connectors:read");
    assert_eq!(
        kit.start(&read_only, ws).await.status,
        StatusCode::FORBIDDEN
    );

    // Extending someone else's connection: not found.
    let foreign = kit
        .post(
            &ana,
            &uri,
            serde_json::json!({"provider": "google", "capabilities": ["messages"],
                               "connection": crate::ConnectionId::new()}),
        )
        .await;
    assert_eq!(foreign.status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn without_a_configured_provider_nothing_connects(db: PgPool) {
    let kit = Kit::inert(db).await;
    let ws = WorkspaceId::new();
    let ana = kit.user("ana");
    kit.join(&ana, ws, Role::Member);
    let started = kit.start(&ana, ws).await;
    assert_eq!(started.status, StatusCode::CONFLICT);
    assert_eq!(started.json["code"], "provider_not_configured");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_unverified_address_is_kept_as_unverified(db: PgPool) {
    let kit = Kit::new(db).await;
    kit.google.unverified_email();
    let ws = WorkspaceId::new();
    let ana = kit.user("ana");
    kit.join(&ana, ws, Role::Member);
    let connection = kit.connect(&ana, ws).await;
    assert_eq!(connection["address_verified"], false);
    let granted = kit
        .payloads(ws, "connectors.capability_granted")
        .await
        .remove(0);
    let account: BlobRef = serde_json::from_value(granted["account"].clone()).unwrap();
    let account: serde_json::Value = serde_json::from_slice(&kit.blob(ws, &account).await).unwrap();
    assert_contract("connection-account.v1.schema.json", &account);
    assert_eq!(account["address_verified"], false);
}
