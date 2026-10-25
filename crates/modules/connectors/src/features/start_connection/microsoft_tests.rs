//! Connecting Outlook (campbooks-cloud#315): the same front-door rules as
//! Gmail, the mailbox read from Graph `/me`, and nothing offered while
//! Microsoft mail is off.

use axum::http::StatusCode;
use campbooks_kernel::{
    Error,
    blobs::BlobRef,
    tenancy::{Role, WorkspaceId},
};
use sqlx::PgPool;

use crate::{
    microsoft::MAIL_SCOPES,
    testkit::{Kit, assert_contract},
    testkit_microsoft::{ADDRESS, GRAPH_ID, SUB},
};

/// Every mail scope, as Microsoft may answer them (short names).
const GRANTED: [&str; 4] = ["Mail.ReadWrite", "Mail.Send", "User.Read", "offline_access"];

fn member(kit: &Kit, name: &str, ws: WorkspaceId) -> crate::testkit::Caller {
    let caller = kit.user(name);
    kit.join(&caller, ws, Role::Member);
    caller
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_member_connects_outlook(db: PgPool) {
    let kit = Kit::with_microsoft(db).await;
    let ws = WorkspaceId::new();
    let ana = member(&kit, "ana", ws);

    // The front door is asked for Microsoft's mail scopes.
    let intake = crate::grant_intake(kit.deps.clone());
    assert_eq!(intake.mailbox_scopes("microsoft"), MAIL_SCOPES);
    assert!(
        intake
            .capability_scopes("microsoft", &["calendar".to_owned()])
            .is_empty()
    );
    let started = kit
        .post(
            &ana,
            &format!("/api/v1/w/{ws}/connections/authorizations"),
            serde_json::json!({"provider": "microsoft", "capabilities": ["messages"]}),
        )
        .await;
    assert_eq!(started.status, StatusCode::CREATED, "{:?}", started.json);
    assert_eq!(
        started.json["authorize_url"],
        "/auth/start/microsoft?link=fake-ticket"
    );
    for capability in ["calendar", "files"] {
        let refused = kit
            .post(
                &ana,
                &format!("/api/v1/w/{ws}/connections/authorizations"),
                serde_json::json!({"provider": "microsoft", "capabilities": [capability]}),
            )
            .await;
        assert_eq!(refused.status, StatusCode::BAD_REQUEST, "{capability}");
        assert_eq!(refused.json["code"], "capability_not_offered");
    }

    let connection = kit.connect_microsoft(&ana, ws).await;
    assert_eq!(connection["provider"], "microsoft");
    // The mailbox's own address, from Graph, not the ID token's claim.
    assert_eq!(connection["address"], ADDRESS);
    assert_eq!(connection["address_verified"], true);
    assert_eq!(connection["display_name"], "Ana Rivera");
    assert_eq!(connection["capabilities"], serde_json::json!(["messages"]));
    assert_eq!(connection["health"], "ok");
    assert_eq!(connection["streams"][0]["capability"], "messages");
    let (subject, key, address): (String, String, String) = sqlx::query_as(
        "SELECT c.account_subject, s.mailbox_key, s.address
           FROM connectors.connections c JOIN connectors.streams s ON s.connection_id = c.id",
    )
    .fetch_one(&kit.db)
    .await
    .unwrap();
    assert_eq!((subject.as_str(), key.as_str()), (SUB, GRAPH_ID));
    assert_eq!(address, ADDRESS);

    // Sealed at rest, never in the answer.
    let text = connection.to_string();
    assert!(!text.contains("fake-ms-"), "{text}");
    let (refresh, access): (Option<Vec<u8>>, Option<Vec<u8>>) =
        sqlx::query_as("SELECT refresh_token, access_token FROM connectors.credentials")
            .fetch_one(&kit.db)
            .await
            .unwrap();
    for bytes in [refresh.unwrap(), access.unwrap()] {
        assert!(!String::from_utf8_lossy(&bytes).contains("fake-ms-"));
    }

    assert_eq!(
        kit.events_of(ws).await,
        [
            "connectors.connection_added",
            "connectors.capability_granted"
        ]
    );
    let added = kit
        .payloads(ws, "connectors.connection_added")
        .await
        .remove(0);
    assert_eq!(added["provider"], "microsoft");
    let granted = kit
        .payloads(ws, "connectors.capability_granted")
        .await
        .remove(0);
    assert_contract("connectors.capability_granted.v1.schema.json", &granted);
    let account: BlobRef = serde_json::from_value(granted["account"].clone()).unwrap();
    let account: serde_json::Value = serde_json::from_slice(&kit.blob(ws, &account).await).unwrap();
    assert_contract("connection-account.v1.schema.json", &account);
    assert_eq!(account["provider"], "microsoft");
    assert_eq!(account["address"], ADDRESS);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_same_account_again_keeps_one_connection_and_one_mailbox(db: PgPool) {
    let kit = Kit::with_microsoft(db).await;
    let ws = WorkspaceId::new();
    let ana = member(&kit, "ana", ws);
    let first = kit.connect_microsoft(&ana, ws).await;
    let second = kit.connect_microsoft(&ana, ws).await;
    assert_eq!(first["id"], second["id"]);
    let streams: i64 = sqlx::query_scalar("SELECT count(*) FROM connectors.streams")
        .fetch_one(&kit.db)
        .await
        .unwrap();
    assert_eq!(streams, 1);
    assert_eq!(
        kit.payloads(ws, "connectors.capability_granted")
            .await
            .len(),
        1
    );

    // Another member connecting the same account is refused.
    let bea = member(&kit, "bea", ws);
    let taken = kit
        .adopt_microsoft(&bea, ws, SUB, true, &GRANTED, None)
        .await
        .unwrap_err();
    assert_eq!(taken.code(), "account_connected_by_another_member");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_reconnect_is_for_its_own_account_and_mailbox_only(db: PgPool) {
    let kit = Kit::with_microsoft(db).await;
    let ws = WorkspaceId::new();
    let ana = member(&kit, "ana", ws);
    let id = kit.connect_microsoft(&ana, ws).await["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let intake = crate::grant_intake(kit.deps.clone());
    let target = intake
        .reconnect_target(&kit.state, "ana", ws, &id)
        .await
        .unwrap();
    assert_eq!(target.provider, "microsoft");
    assert_eq!(target.account_subject, SUB);
    assert_eq!(target.address, ADDRESS);

    // The same account re-authorises it.
    assert_eq!(
        kit.adopt_microsoft(&ana, ws, SUB, true, &GRANTED, Some(id.clone()))
            .await
            .unwrap(),
        id
    );
    // Another Microsoft account is refused, and so is the same account
    // naming another mailbox than the one connected.
    let other = kit
        .adopt_microsoft(&ana, ws, "another-sub", true, &GRANTED, Some(id.clone()))
        .await
        .unwrap_err();
    assert_eq!(other.code(), "account_mismatch");
    kit.microsoft.answer_me(serde_json::json!({
        "id": "ffffffff-1c2b-4f6a-9e8d-7c6b5a4f3e2d",
        "mail": "someone.else@example.com"
    }));
    let elsewhere = kit
        .adopt_microsoft(&ana, ws, SUB, true, &GRANTED, Some(id.clone()))
        .await
        .unwrap_err();
    assert_eq!(elsewhere.code(), "account_mismatch");
    let elsewhere = kit
        .adopt_microsoft(&ana, ws, SUB, true, &GRANTED, None)
        .await
        .unwrap_err();
    assert_eq!(elsewhere.code(), "account_mismatch");
    // Nothing changed.
    let (address, key): (String, String) = sqlx::query_as(
        "SELECT c.address, s.mailbox_key
           FROM connectors.connections c JOIN connectors.streams s ON s.connection_id = c.id",
    )
    .fetch_one(&kit.db)
    .await
    .unwrap();
    assert_eq!((address.as_str(), key.as_str()), (ADDRESS, GRAPH_ID));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_grant_without_mail_or_a_mailbox_connects_nothing(db: PgPool) {
    let kit = Kit::with_microsoft(db).await;
    let ws = WorkspaceId::new();
    let ana = member(&kit, "ana", ws);
    for scopes in [
        &["Mail.ReadWrite", "User.Read", "offline_access"][..],
        &["Mail.Read", "Mail.Send", "User.Read"][..],
        &[][..],
    ] {
        let refused = kit
            .adopt_microsoft(&ana, ws, SUB, true, scopes, None)
            .await
            .unwrap_err();
        assert_eq!(refused.code(), "consent_incomplete", "{scopes:?}");
    }
    // Graph names no usable mailbox.
    kit.microsoft
        .answer_me(serde_json::json!({"id": GRAPH_ID, "mail": null, "userPrincipalName": null}));
    let none = kit
        .adopt_microsoft(&ana, ws, SUB, true, &GRANTED, None)
        .await
        .unwrap_err();
    assert_eq!(none.code(), "no_mailbox");
    kit.microsoft
        .answer_me(serde_json::json!({"id": "../me", "mail": ADDRESS}));
    let forged = kit
        .adopt_microsoft(&ana, ws, SUB, true, &GRANTED, None)
        .await
        .unwrap_err();
    assert_eq!(forged.code(), "no_mailbox");
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM connectors.connections")
        .fetch_one(&kit.db)
        .await
        .unwrap();
    assert_eq!(rows, 0);
    assert!(kit.events_of(ws).await.is_empty());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_address_is_verified_only_where_it_was_proven(db: PgPool) {
    let kit = Kit::with_microsoft(db).await;
    let ws = WorkspaceId::new();
    let ana = member(&kit, "ana", ws);
    // The ID token's address was not verified (no `xms_edov`).
    let id = kit
        .adopt_microsoft(&ana, ws, SUB, false, &GRANTED, None)
        .await
        .unwrap();
    let read = kit
        .get(&ana, &format!("/api/v1/w/{ws}/connections/{id}"))
        .await;
    assert_eq!(read.json["address_verified"], false);

    // Verified, but for another address than the mailbox's.
    let ws2 = WorkspaceId::new();
    kit.join(&ana, ws2, Role::Member);
    kit.microsoft.answer_me(serde_json::json!({
        "id": GRAPH_ID,
        "mail": "shared.box@example.com",
        "displayName": "Shared"
    }));
    let id = kit
        .adopt_microsoft(&ana, ws2, SUB, true, &GRANTED, None)
        .await
        .unwrap();
    let read = kit
        .get(&ana, &format!("/api/v1/w/{ws2}/connections/{id}"))
        .await;
    assert_eq!(read.json["address"], "shared.box@example.com");
    assert_eq!(read.json["address_verified"], false);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_held_grant_whose_access_token_expired_is_refreshed_once(db: PgPool) {
    let kit = Kit::with_microsoft(db).await;
    let ws = WorkspaceId::new();
    let ana = member(&kit, "ana", ws);
    // A sign-up's grant, adopted after its access token stopped working.
    let before = kit.microsoft.token_calls();
    let (access, refresh) = kit.microsoft.grant();
    kit.microsoft.expire_access_tokens();
    let id = crate::grant_intake(kit.deps.clone())
        .adopt(
            &kit.state,
            campbooks_kernel::connections::ProviderGrant {
                workspace: ws,
                subject: ana.subject.clone(),
                provider: "microsoft".to_owned(),
                account_subject: SUB.to_owned(),
                issuer:
                    "https://login.microsoftonline.com/9188040d-6c67-4c5b-b112-36a304b66dad/v2.0"
                        .to_owned(),
                address: ADDRESS.to_owned(),
                address_verified: false,
                display_name: None,
                scopes: GRANTED.map(str::to_owned).to_vec(),
                access_token: secrecy::SecretString::from(access),
                refresh_token: Some(secrecy::SecretString::from(refresh)),
                expires_in: Some(10),
                extending: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(kit.microsoft.token_calls(), before + 1);
    assert!(!id.is_empty());
    // A grant Microsoft no longer honours connects nothing.
    let ws2 = WorkspaceId::new();
    kit.join(&ana, ws2, Role::Member);
    kit.microsoft.refuse_refresh(
        400,
        serde_json::json!({"error": "invalid_grant", "error_codes": [700_082]}),
    );
    let (access, refresh) = kit.microsoft.grant();
    kit.microsoft.expire_access_tokens();
    let refused = crate::grant_intake(kit.deps.clone())
        .adopt(
            &kit.state,
            campbooks_kernel::connections::ProviderGrant {
                workspace: ws2,
                subject: ana.subject.clone(),
                provider: "microsoft".to_owned(),
                account_subject: SUB.to_owned(),
                issuer:
                    "https://login.microsoftonline.com/9188040d-6c67-4c5b-b112-36a304b66dad/v2.0"
                        .to_owned(),
                address: ADDRESS.to_owned(),
                address_verified: false,
                display_name: None,
                scopes: GRANTED.map(str::to_owned).to_vec(),
                access_token: secrecy::SecretString::from(access),
                refresh_token: Some(secrecy::SecretString::from(refresh)),
                expires_in: Some(10),
                extending: None,
            },
        )
        .await
        .unwrap_err();
    assert_eq!(refused.code(), "consent_incomplete");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn microsoft_is_not_offered_while_its_mail_is_off(db: PgPool) {
    // The kit's default: Microsoft mail off, as on an instance with the
    // sign-in client alone.
    let kit = Kit::new(db).await;
    assert!(kit.deps.microsoft.is_none());
    let ws = WorkspaceId::new();
    let ana = member(&kit, "ana", ws);
    let intake = crate::grant_intake(kit.deps.clone());
    assert!(intake.mailbox_scopes("microsoft").is_empty());
    let started = kit
        .post(
            &ana,
            &format!("/api/v1/w/{ws}/connections/authorizations"),
            serde_json::json!({"provider": "microsoft", "capabilities": ["messages"]}),
        )
        .await;
    assert_eq!(started.status, StatusCode::CONFLICT);
    assert_eq!(started.json["code"], "provider_not_configured");
    assert!(kit.door.asked().is_empty());
    // A grant that reaches the intake anyway connects nothing.
    let refused = kit
        .adopt_microsoft(&ana, ws, SUB, true, &GRANTED, None)
        .await
        .unwrap_err();
    assert!(matches!(refused, Error::Conflict(_)));
    assert_eq!(refused.code(), "provider_not_configured");
    assert!(kit.microsoft.calls().is_empty(), "Microsoft was not asked");
}
