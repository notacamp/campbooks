//! Asking for an address check again: who may, and on what.

use axum::http::StatusCode;
use campbooks_kernel::tenancy::{Role, WorkspaceId};
use serde_json::json;
use sqlx::PgPool;

use crate::{
    imap::{Security, Servers},
    testkit::Kit,
    testkit_imap::{self, FakeServer, IMAP_HOST, SMTP_HOST, Script},
};

fn uri(ws: WorkspaceId, connection: &str) -> String {
    format!("/api/v1/w/{ws}/connections/{connection}/address-check")
}

#[sqlx::test(migrations = "../../../migrations")]
async fn only_who_may_see_the_mailbox_asks(db: PgPool) {
    let (kit, fakes) = testkit_imap::kit(db).await;
    let ws = WorkspaceId::new();
    let ana = kit.user_signed_in("ana", 5);
    kit.join(&ana, ws, Role::Member);
    let imap = FakeServer::imap(Security::Tls, Script::accepting("ana", "pw")).await;
    let smtp = FakeServer::smtp(Security::Starttls, Script::accepting("ana", "pw")).await;
    fakes.resolver.name(IMAP_HOST, &["127.0.0.1"]);
    fakes.resolver.name(SMTP_HOST, &["127.0.0.1"]);
    let servers = Servers {
        imap: imap.at(IMAP_HOST),
        smtp: smtp.at(SMTP_HOST),
    };
    let connected = kit
        .post(
            &ana,
            &format!("/api/v1/w/{ws}/connections/imap"),
            json!({"address": "ana@bakery.example", "username": "ana", "password": "pw",
                   "servers": servers}),
        )
        .await;
    assert_eq!(
        connected.status,
        StatusCode::CREATED,
        "{:?}",
        connected.json
    );
    let id = connected.json["id"].as_str().unwrap().to_owned();

    // Another member does not see Ana's mailbox.
    let bea = kit.user_signed_in("bea", 5);
    kit.join(&bea, ws, Role::Member);
    let refused = kit.post(&bea, &uri(ws, &id), json!(null)).await;
    assert_eq!(refused.status, StatusCode::NOT_FOUND);
    // Nor does anyone outside the workspace.
    let eve = kit.user_signed_in("eve", 5);
    let refused = kit.post(&eve, &uri(ws, &id), json!(null)).await;
    assert_eq!(refused.status, StatusCode::NOT_FOUND);
    // No such connection.
    let refused = kit
        .post(
            &ana,
            &uri(ws, &crate::ConnectionId::new().to_string()),
            json!(null),
        )
        .await;
    assert_eq!(refused.status, StatusCode::NOT_FOUND);

    // Ana does: the check under way goes on.
    let asked = kit.post(&ana, &uri(ws, &id), json!(null)).await;
    assert_eq!(asked.status, StatusCode::ACCEPTED, "{:?}", asked.json);
    assert_eq!(asked.json["address_check"], "checking");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_mailbox_that_is_not_imap_needs_no_check(db: PgPool) {
    let kit = Kit::new(db).await;
    let ws = WorkspaceId::new();
    let ana = kit.user("ana");
    kit.join(&ana, ws, Role::Member);
    let connection = kit.connect(&ana, ws).await;
    let id = connection["id"].as_str().unwrap();
    assert!(connection["address_check"].is_null());
    let asked = kit.post(&ana, &uri(ws, id), json!(null)).await;
    assert_eq!(asked.status, StatusCode::CONFLICT);
    assert_eq!(asked.json["code"], "address_check_not_needed");
}
