//! Connecting an IMAP mailbox: signed in to its servers before anything is
//! kept, over TLS only, at checked addresses, with refusals that never
//! repeat a server's words, and limits that keep it from being a
//! password-guessing service.

use axum::http::StatusCode;
use campbooks_kernel::{
    blobs::BlobRef,
    tenancy::{Role, WorkspaceId},
    testing::TestResponse,
};
use sqlx::PgPool;

use crate::{
    domain::{
        credentials::{self, TokenError},
        model::ConnectionId,
    },
    imap::{Security, Servers},
    testkit::{Caller, Kit, assert_contract},
    testkit_imap::{self, FakeServer, IMAP_HOST, SMTP_HOST, Script},
};

fn connect_uri(ws: WorkspaceId) -> String {
    format!("/api/v1/w/{ws}/connections/imap")
}

fn body(address: &str, password: &str, servers: &Servers) -> serde_json::Value {
    serde_json::json!({
        "address": address,
        "password": password,
        "servers": servers,
    })
}

async fn connect(
    kit: &Kit,
    caller: &Caller,
    ws: WorkspaceId,
    body: serde_json::Value,
) -> TestResponse {
    kit.post(caller, &connect_uri(ws), body).await
}

async fn count(kit: &Kit, table: &str) -> i64 {
    sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "SELECT count(*) FROM connectors.{table}"
    )))
    .fetch_one(&kit.db)
    .await
    .unwrap()
}

/// A member of a fresh workspace who signed in five minutes ago.
fn member(kit: &Kit, sub: &str) -> (Caller, WorkspaceId) {
    let ws = WorkspaceId::new();
    let caller = kit.user_signed_in(sub, 5);
    kit.join(&caller, ws, Role::Member);
    (caller, ws)
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_app_password_connects_a_mailbox(db: PgPool) {
    let (kit, fakes) = testkit_imap::kit(db).await;
    let (ana, ws) = member(&kit, "ana");
    let (imap, smtp, servers) =
        testkit_imap::mailbox(&fakes, "ana@yahoo.com", "abcd efgh ijkl mnop").await;

    let connected = connect(
        &kit,
        &ana,
        ws,
        body("ana@yahoo.com", "abcd efgh ijkl mnop", &servers),
    )
    .await;
    assert_eq!(
        connected.status,
        StatusCode::CREATED,
        "{:?}",
        connected.json
    );
    let connection = &connected.json;
    assert_eq!(connection["provider"], "imap");
    assert_eq!(connection["address"], "ana@yahoo.com");
    assert_eq!(connection["address_verified"], true);
    assert_eq!(connection["capabilities"], serde_json::json!(["messages"]));
    assert_eq!(connection["health"], "ok");
    let stream = &connection["streams"][0];
    assert_eq!(stream["capability"], "messages");
    assert!(stream["inbox_id"].is_string());
    // Never the password, in the answer or anywhere it was sent.
    assert!(
        !connection.to_string().contains("abcd efgh"),
        "{connection}"
    );

    // Both servers were signed in to, over TLS, and nothing was sent.
    assert!(imap.seen().listed);
    assert_eq!(smtp.seen().sign_ins, 1);
    assert!(!imap.seen().password_in_clear && !smtp.seen().password_in_clear);
    assert!(!smtp.seen().mail_from);

    // The password is sealed, and no token is kept.
    let (password, refresh): (Option<Vec<u8>>, Option<Vec<u8>>) =
        sqlx::query_as("SELECT password, refresh_token FROM connectors.credentials")
            .fetch_one(&kit.db)
            .await
            .unwrap();
    assert!(refresh.is_none());
    let sealed = password.unwrap();
    assert!(!String::from_utf8_lossy(&sealed).contains("abcd"));
    let (username, host, source): (String, String, String) =
        sqlx::query_as("SELECT username, imap_host, source FROM connectors.imap_servers")
            .fetch_one(&kit.db)
            .await
            .unwrap();
    assert_eq!(
        (username.as_str(), host.as_str(), source.as_str()),
        ("ana@yahoo.com", IMAP_HOST, "manual")
    );

    // Announced like any connection; its stream starts.
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
    assert_eq!(added["provider"], "imap");
    assert_eq!(added["account_subject"], "ana@yahoo.com@imap.example.com");
    let granted = kit
        .payloads(ws, "connectors.capability_granted")
        .await
        .remove(0);
    assert_contract("connectors.capability_granted.v1.schema.json", &granted);
    let account: BlobRef = serde_json::from_value(granted["account"].clone()).unwrap();
    let account: serde_json::Value = serde_json::from_slice(&kit.blob(ws, &account).await).unwrap();
    assert_contract("connection-account.v1.schema.json", &account);
    assert_eq!(account["provider"], "imap");

    // Its stream waits (reading lands with campbooks-cloud#311), healthy.
    kit.run_jobs().await;
    let view = kit
        .get(
            &ana,
            &format!(
                "/api/v1/w/{ws}/connections/{}",
                connection["id"].as_str().unwrap()
            ),
        )
        .await;
    assert_eq!(view.json["health"], "ok", "{:?}", view.json);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_wrong_password_keeps_nothing_and_never_repeats_the_server(db: PgPool) {
    let (kit, fakes) = testkit_imap::kit(db).await;
    let (ana, ws) = member(&kit, "ana");
    let (imap, _smtp, servers) = testkit_imap::mailbox(&fakes, "ana@yahoo.com", "right").await;

    let refused = connect(&kit, &ana, ws, body("ana@yahoo.com", "wrong", &servers)).await;
    assert_eq!(
        refused.status,
        StatusCode::BAD_REQUEST,
        "{:?}",
        refused.json
    );
    assert_eq!(refused.json["code"], "imap_login_refused");
    let detail = refused.json["detail"].as_str().unwrap();
    // Yahoo asks for an app password: the answer says so.
    assert!(detail.contains("app password"), "{detail}");
    // The server's own words (which name the username) are not repeated.
    assert!(!refused.json.to_string().contains("may not sign in"));
    assert!(!refused.json.to_string().contains("ana@yahoo.com"));
    assert_eq!(imap.seen().sign_ins, 1);
    for table in ["connections", "credentials", "imap_servers", "streams"] {
        assert_eq!(count(&kit, table).await, 0, "{table}");
    }

    // A provider not known to want one gets the general advice.
    let (_imap, _smtp, other) = testkit_imap::mailbox(&fakes, "bob@bakery.example", "right").await;
    let general = connect(&kit, &ana, ws, body("bob@bakery.example", "wrong", &other)).await;
    assert_eq!(general.json["code"], "imap_login_refused");
    assert_ne!(general.json["detail"], refused.json["detail"]);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn no_password_is_sent_without_tls(db: PgPool) {
    let (kit, fakes) = testkit_imap::kit(db).await;
    let (ana, ws) = member(&kit, "ana");
    fakes.resolver.name(IMAP_HOST, &["127.0.0.1"]);
    fakes.resolver.name(SMTP_HOST, &["127.0.0.1"]);
    let plain_only = Script {
        starttls: false,
        ..Script::accepting("ana@bakery.example", "pw")
    };
    let imap = FakeServer::imap(Security::Starttls, plain_only.clone()).await;
    let smtp = FakeServer::smtp(Security::Tls, Script::accepting("ana@bakery.example", "pw")).await;
    let servers = Servers {
        imap: imap.at(IMAP_HOST),
        smtp: smtp.at(SMTP_HOST),
    };
    let refused = connect(&kit, &ana, ws, body("ana@bakery.example", "pw", &servers)).await;
    assert_eq!(refused.status, StatusCode::BAD_REQUEST);
    assert_eq!(refused.json["code"], "starttls_missing");
    assert_eq!(imap.seen().sign_ins, 0);
    assert!(!imap.seen().password_in_clear);

    // The same on the SMTP side.
    let imap = FakeServer::imap(Security::Tls, Script::accepting("ana@bakery.example", "pw")).await;
    let smtp = FakeServer::smtp(Security::Starttls, plain_only).await;
    let servers = Servers {
        imap: imap.at(IMAP_HOST),
        smtp: smtp.at(SMTP_HOST),
    };
    let refused = connect(&kit, &ana, ws, body("ana@bakery.example", "pw", &servers)).await;
    assert_eq!(refused.json["code"], "starttls_missing");
    assert_eq!(smtp.seen().sign_ins, 0);
    assert_eq!(count(&kit, "connections").await, 0);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_starttls_imap_server_and_a_tls_smtp_server_connect(db: PgPool) {
    let (kit, fakes) = testkit_imap::kit(db).await;
    let (ana, ws) = member(&kit, "ana");
    fakes.resolver.name(IMAP_HOST, &["127.0.0.1"]);
    fakes.resolver.name(SMTP_HOST, &["127.0.0.1"]);
    // No `AUTH=PLAIN`: `LOGIN`, after STARTTLS.
    let imap = FakeServer::imap(
        Security::Starttls,
        Script {
            auth: false,
            ..Script::accepting("ana", "pw")
        },
    )
    .await;
    let smtp = FakeServer::smtp(Security::Tls, Script::accepting("ana", "pw")).await;
    let servers = Servers {
        imap: imap.at(IMAP_HOST),
        smtp: smtp.at(SMTP_HOST),
    };
    let mut request = body("ana@bakery.example", "pw", &servers);
    request["username"] = "ana".into();
    let connected = connect(&kit, &ana, ws, request).await;
    assert_eq!(
        connected.status,
        StatusCode::CREATED,
        "{:?}",
        connected.json
    );
    // A login proves the username, not the address.
    assert_eq!(connected.json["address_verified"], false);
    assert!(!imap.seen().password_in_clear && !smtp.seen().password_in_clear);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn refusals_name_what_went_wrong(db: PgPool) {
    let (kit, fakes) = testkit_imap::kit(db).await;
    let (ana, ws) = member(&kit, "ana");
    let (_imap, smtp, servers) = testkit_imap::mailbox(&fakes, "ana@bakery.example", "pw").await;
    let attempt = |servers: Servers, password: &'static str| {
        let kit = &kit;
        let ana = &ana;
        async move {
            connect(kit, ana, ws, body("ana@bakery.example", password, &servers))
                .await
                .json["code"]
                .clone()
        }
    };

    // A certificate that is not the host's.
    fakes.resolver.name("imap.example.net", &["127.0.0.1"]);
    let mut wrong_name = servers.clone();
    wrong_name.imap.host = "imap.example.net".to_owned();
    assert_eq!(attempt(wrong_name, "pw").await, "imap_tls_failed");

    // A metadata address, whatever the instance allows: no connection.
    fakes
        .resolver
        .name("imap.metadata.example", &["169.254.169.254"]);
    let mut metadata = servers.clone();
    metadata.imap.host = "imap.metadata.example".to_owned();
    assert_eq!(attempt(metadata, "pw").await, "host_refused");

    // A name that does not resolve.
    let mut nowhere = servers.clone();
    nowhere.imap.host = "imap.nowhere.example".to_owned();
    assert_eq!(attempt(nowhere, "pw").await, "imap_unreachable");

    // Not an IMAP server.
    let web = FakeServer::imap(
        Security::Tls,
        Script {
            not_mail: true,
            ..Script::accepting("ana@bakery.example", "pw")
        },
    )
    .await;
    let mut not_imap = servers.clone();
    not_imap.imap = web.at(IMAP_HOST);
    assert_eq!(attempt(not_imap, "pw").await, "imap_not_an_imap_server");

    // `LOGINDISABLED` and no other way in: refused, nothing sent.
    let disabled = FakeServer::imap(
        Security::Tls,
        Script {
            auth: false,
            login_disabled: true,
            ..Script::accepting("ana@bakery.example", "pw")
        },
    )
    .await;
    let mut no_login = servers.clone();
    no_login.imap = disabled.at(IMAP_HOST);
    assert_eq!(attempt(no_login, "pw").await, "imap_login_refused");
    assert_eq!(disabled.seen().sign_ins, 0);

    // The SMTP server refuses the password.
    smtp.set_password("another");
    assert_eq!(attempt(servers, "pw").await, "smtp_login_refused");
    assert_eq!(count(&kit, "connections").await, 0);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_native_providers_mailbox_is_connected_natively(db: PgPool) {
    let (kit, fakes) = testkit_imap::kit(db).await;
    let (ana, ws) = member(&kit, "ana");
    let (_imap, _smtp, servers) = testkit_imap::mailbox(&fakes, "ana@gmail.com", "pw").await;
    let gmail = connect(&kit, &ana, ws, body("ana@gmail.com", "pw", &servers)).await;
    assert_eq!(gmail.status, StatusCode::BAD_REQUEST);
    assert_eq!(gmail.json["code"], "use_native_provider");
    assert!(gmail.json["detail"].as_str().unwrap().contains("Google"));

    let mut at_google = servers.clone();
    at_google.imap.host = "imap.gmail.com".to_owned();
    let refused = connect(&kit, &ana, ws, body("ana@bakery.example", "pw", &at_google)).await;
    assert_eq!(refused.json["code"], "use_native_provider");

    // Nothing found and no servers given: the member is asked for them.
    let unknown = connect(
        &kit,
        &ana,
        ws,
        serde_json::json!({"address": "ana@nowhere.example", "password": "pw"}),
    )
    .await;
    assert_eq!(unknown.json["code"], "imap_servers_required");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn only_a_recently_signed_in_member_on_an_instance_with_imap(db: PgPool) {
    let (kit, fakes) = testkit_imap::kit(db.clone()).await;
    let ws = WorkspaceId::new();
    let (imap, _smtp, servers) = testkit_imap::mailbox(&fakes, "ana@bakery.example", "pw").await;
    let request = body("ana@bakery.example", "pw", &servers);

    let stale = kit.user_signed_in("ana", 11);
    kit.join(&stale, ws, Role::Member);
    let refused = connect(&kit, &stale, ws, request.clone()).await;
    assert_eq!(refused.status, StatusCode::FORBIDDEN);
    assert_eq!(refused.json["code"], "step_up_required");

    let client = kit.client(ws);
    let refused = connect(&kit, &client, ws, request.clone()).await;
    assert_eq!(refused.status, StatusCode::FORBIDDEN);
    assert_eq!(refused.json["code"], "connection_needs_user");
    assert_eq!(imap.seen().connections, 0);

    // Off (the default): neither IMAP operation, and no consent for `imap`.
    let off = Kit::new(db).await;
    let bea = off.user_signed_in("bea", 2);
    off.join(&bea, ws, Role::Member);
    let refused = connect(&off, &bea, ws, request).await;
    assert_eq!(refused.status, StatusCode::CONFLICT);
    assert_eq!(refused.json["code"], "provider_not_configured");
    let consent = off
        .post(
            &bea,
            &format!("/api/v1/w/{ws}/connections/authorizations"),
            serde_json::json!({"provider": "imap", "capabilities": ["messages"]}),
        )
        .await;
    assert_eq!(consent.json["code"], "provider_not_configured");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn reconnecting_replaces_the_password(db: PgPool) {
    let (kit, fakes) = testkit_imap::kit(db).await;
    let (ana, ws) = member(&kit, "ana");
    let (imap, smtp, servers) = testkit_imap::mailbox(&fakes, "ana@bakery.example", "old").await;
    let first = connect(&kit, &ana, ws, body("ana@bakery.example", "old", &servers)).await;
    assert_eq!(first.status, StatusCode::CREATED);
    let id = first.json["id"].as_str().unwrap().to_owned();
    let sealed = |kit: &Kit| {
        let db = kit.db.clone();
        async move {
            sqlx::query_scalar::<_, Vec<u8>>("SELECT password FROM connectors.credentials")
                .fetch_one(&db)
                .await
                .unwrap()
        }
    };
    let before = sealed(&kit).await;
    sqlx::query("UPDATE connectors.connections SET health = 'needs_reauth'")
        .execute(&kit.db)
        .await
        .unwrap();

    // The provider changed the password: the grantor connects again.
    imap.set_password("new");
    smtp.set_password("new");
    let mut again = body("ana@bakery.example", "new", &servers);
    again["connection"] = id.clone().into();
    let reconnected = connect(&kit, &ana, ws, again.clone()).await;
    assert_eq!(
        reconnected.status,
        StatusCode::CREATED,
        "{:?}",
        reconnected.json
    );
    assert_eq!(reconnected.json["id"], id.as_str());
    assert_eq!(reconnected.json["health"], "ok");
    // The same stream and inbox.
    assert_eq!(reconnected.json["streams"].as_array().unwrap().len(), 1);
    assert_eq!(
        reconnected.json["streams"][0]["inbox_id"],
        first.json["streams"][0]["inbox_id"]
    );
    assert_ne!(sealed(&kit).await, before);
    assert_eq!(count(&kit, "connections").await, 1);

    // Someone else's connection is not theirs to connect again.
    let bea = kit.user_signed_in("bea", 5);
    kit.join(&bea, ws, Role::Member);
    let refused = connect(&kit, &bea, ws, again).await;
    assert_eq!(refused.status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_mailbox_cannot_be_guessed_at(db: PgPool) {
    let (kit, fakes) = testkit_imap::kit(db).await;
    let (imap, _smtp, servers) = testkit_imap::mailbox(&fakes, "bob@example.net", "secret").await;
    // Five failed sign-ins for one address, from two workspaces.
    let (ana, ws_a) = member(&kit, "ana");
    let (cy, ws_c) = member(&kit, "cy");
    for (who, ws) in [
        (&ana, ws_a),
        (&cy, ws_c),
        (&ana, ws_a),
        (&cy, ws_c),
        (&ana, ws_a),
    ] {
        let refused = connect(&kit, who, ws, body("bob@example.net", "guess", &servers)).await;
        assert_eq!(refused.json["code"], "imap_login_refused");
    }
    let tried = imap.seen().connections;

    // The sixth is refused without contacting the server, even with the
    // right password.
    let locked = connect(&kit, &cy, ws_c, body("bob@example.net", "secret", &servers)).await;
    assert_eq!(locked.status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(locked.json["code"], "too_many_attempts");
    assert_eq!(imap.seen().connections, tried);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_password_moved_to_another_row_does_not_open(db: PgPool) {
    let (kit, fakes) = testkit_imap::kit(db).await;
    let (ana, ws) = member(&kit, "ana");
    let (_imap, _smtp, servers) = testkit_imap::mailbox(&fakes, "ana@bakery.example", "pw-a").await;
    let a = connect(&kit, &ana, ws, body("ana@bakery.example", "pw-a", &servers)).await;
    assert_eq!(a.status, StatusCode::CREATED, "{:?}", a.json);
    let mut second = body("ana@bakery.example", "pw-b", &servers);
    second["username"] = "ana.b@bakery.example".into();
    let fake_b = FakeServer::imap(
        Security::Tls,
        Script::accepting("ana.b@bakery.example", "pw-b"),
    )
    .await;
    let smtp_b = FakeServer::smtp(
        Security::Starttls,
        Script::accepting("ana.b@bakery.example", "pw-b"),
    )
    .await;
    second["servers"] = serde_json::to_value(Servers {
        imap: fake_b.at(IMAP_HOST),
        smtp: smtp_b.at(SMTP_HOST),
    })
    .unwrap();
    let b = connect(&kit, &ana, ws, second).await;
    assert_eq!(b.status, StatusCode::CREATED, "{:?}", b.json);
    let a: ConnectionId = serde_json::from_value(a.json["id"].clone()).unwrap();
    let b: ConnectionId = serde_json::from_value(b.json["id"].clone()).unwrap();

    // A's sealed password copied into B's row.
    sqlx::query(
        "UPDATE connectors.credentials SET password =
             (SELECT password FROM connectors.credentials WHERE connection_id = $1)
          WHERE connection_id = $2",
    )
    .bind(a)
    .bind(b)
    .execute(&kit.db)
    .await
    .unwrap();
    let cipher = kit.deps.cipher.clone().unwrap();
    let mut conn = kit.db.acquire().await.unwrap();
    assert!(credentials::password(&mut conn, &cipher, a).await.is_ok());
    assert!(matches!(
        credentials::password(&mut conn, &cipher, b).await,
        Err(TokenError::NeedsReauth("credentials_unreadable"))
    ));
    drop(conn);

    // Its stream's run finds out: B needs connecting again, A does not.
    kit.run_jobs().await;
    let health = |id: ConnectionId| {
        let db = kit.db.clone();
        async move {
            sqlx::query_scalar::<_, String>(
                "SELECT health FROM connectors.connections WHERE id = $1",
            )
            .bind(id)
            .fetch_one(&db)
            .await
            .unwrap()
        }
    };
    assert_eq!(health(a).await, "ok");
    assert_eq!(health(b).await, "needs_reauth");
}
