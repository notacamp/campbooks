//! Disconnecting revokes the grant and deletes what the connection holds.

use axum::http::StatusCode;
use campbooks_kernel::clock::Clock as _;
use campbooks_kernel::tenancy::{Role, WorkspaceId};
use chrono::TimeDelta;
use sqlx::PgPool;

use crate::testkit::{Kit, synthetic};

#[sqlx::test(migrations = "../../../migrations")]
async fn the_grantor_disconnects_and_the_grant_is_revoked(db: PgPool) {
    let kit = Kit::new(db).await;
    kit.google.seed(
        &synthetic(1),
        &["INBOX"],
        kit.clock.now() - TimeDelta::hours(1),
    );
    let ws = WorkspaceId::new();
    let ana = kit.user("ana");
    kit.join(&ana, ws, Role::Member);
    let bea = kit.user("bea");
    kit.join(&bea, ws, Role::Member);
    let connection = kit.connect(&ana, ws).await;
    kit.run_jobs().await;
    let id = connection["id"].as_str().unwrap();
    let uri = format!("/api/v1/w/{ws}/connections/{id}");

    // Someone who cannot see it, or a client token, cannot disconnect it.
    assert_eq!(kit.delete(&bea, &uri).await.status, StatusCode::NOT_FOUND);
    let client = kit.delete(&kit.client(ws), &uri).await;
    assert_eq!(client.status, StatusCode::FORBIDDEN);
    assert_eq!(client.json["code"], "connection_needs_user");

    assert_eq!(kit.delete(&ana, &uri).await.status, StatusCode::NO_CONTENT);
    let revoked = kit.google.revoked();
    assert_eq!(revoked.len(), 1);
    assert!(revoked[0].starts_with("fake-refresh"));
    let removed = kit.payloads(ws, "connectors.connection_removed").await;
    assert_eq!(removed.len(), 1);
    assert_eq!(
        removed[0]["inbox_ids"][0],
        connection["streams"][0]["inbox_id"]
    );
    // Deleting its mail is the default.
    assert_eq!(removed[0]["mail"], "delete");
    let left: i64 = sqlx::query_scalar!(
        r#"SELECT (SELECT count(*) FROM connectors.connections)
                + (SELECT count(*) FROM connectors.credentials)
                + (SELECT count(*) FROM connectors.streams)
                + (SELECT count(*) FROM connectors.sync_due)
                + (SELECT count(*) FROM connectors.messages)
                + (SELECT count(*) FROM connectors.batches)
                + (SELECT count(*) FROM kernel.blob_refs WHERE owner_type LIKE 'connectors.%')
           AS "n!""#
    )
    .fetch_one(&kit.db)
    .await
    .unwrap();
    assert_eq!(left, 0);
    // The client change feed heard of it: added, then deleted (ids only).
    let feed: Vec<String> = sqlx::query_scalar!(
        "SELECT op FROM kernel.changes
          WHERE record_type = 'connectors.connection' AND record_id = $1 ORDER BY published",
        id
    )
    .fetch_all(&kit.db)
    .await
    .unwrap();
    assert_eq!(feed.first().map(String::as_str), Some("upsert"));
    assert_eq!(feed.last().map(String::as_str), Some("delete"));
    assert_eq!(kit.get(&ana, &uri).await.status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn only_the_grantor_disconnects_even_when_an_admin_sees_it(db: PgPool) {
    let kit = Kit::new(db).await;
    let ws = WorkspaceId::new();
    let ana = kit.user("ana");
    kit.join(&ana, ws, Role::Member);
    let carl = kit.user("carl");
    kit.join(&carl, ws, Role::Admin);
    let connection = kit.connect(&ana, ws).await;
    let id = connection["id"].as_str().unwrap();
    let uri = format!("/api/v1/w/{ws}/connections/{id}");

    assert_eq!(kit.get(&carl, &uri).await.status, StatusCode::OK);
    let refused = kit.delete(&carl, &uri).await;
    assert_eq!(refused.status, StatusCode::FORBIDDEN);
    assert_eq!(refused.json["code"], "not_your_connection");
    assert!(kit.google.revoked().is_empty());
    assert_eq!(kit.get(&ana, &uri).await.status, StatusCode::OK);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn keeping_the_mail_is_for_your_own_mailbox_or_a_manager(db: PgPool) {
    let kit = Kit::new(db).await;
    let ws = WorkspaceId::new();
    let ana = kit.user("ana");
    kit.join(&ana, ws, Role::Member);
    let carl = kit.user("carl");
    kit.join(&carl, ws, Role::Admin);

    // Ana's shared mailbox (she doesn't sign in with it): delete only.
    let shared = kit
        .adopt_as(&ana, ws, "support-sub", None, false)
        .await
        .unwrap();
    let uri = format!("/api/v1/w/{ws}/connections/{shared}");
    let view = kit.get(&ana, &uri).await;
    assert_eq!(view.json["grantor_signs_in"], false);
    let refused = kit.delete(&ana, &format!("{uri}?mail=keep")).await;
    assert_eq!(refused.status, StatusCode::FORBIDDEN);
    assert_eq!(refused.json["code"], "keep_mail_not_allowed");
    assert_eq!(kit.get(&ana, &uri).await.status, StatusCode::OK);

    // Her own mailbox: she may keep its mail.
    let own = kit.connect(&ana, ws).await;
    let own_id = own["id"].as_str().unwrap();
    assert_eq!(own["grantor_signs_in"], true);
    let kept = kit
        .delete(
            &ana,
            &format!("/api/v1/w/{ws}/connections/{own_id}?mail=keep"),
        )
        .await;
    assert_eq!(kept.status, StatusCode::NO_CONTENT);

    // An admin may keep the mail of a shared mailbox they connected.
    let carls = kit
        .adopt_as(&carl, ws, "billing-sub", None, false)
        .await
        .unwrap();
    let kept = kit
        .delete(
            &carl,
            &format!("/api/v1/w/{ws}/connections/{carls}?mail=keep"),
        )
        .await;
    assert_eq!(kept.status, StatusCode::NO_CONTENT);

    let removed = kit.payloads(ws, "connectors.connection_removed").await;
    let choices: Vec<&str> = removed
        .iter()
        .map(|e| e["mail"].as_str().unwrap())
        .collect();
    assert_eq!(choices, ["keep", "keep"]);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_later_grant_never_clears_that_you_sign_in_with_it(db: PgPool) {
    let kit = Kit::new(db).await;
    let ws = WorkspaceId::new();
    let ana = kit.user("ana");
    kit.join(&ana, ws, Role::Member);
    let id = kit.adopt_as(&ana, ws, "ana-sub", None, true).await.unwrap();
    // A reconnect says nothing about signing in (`false`): the mark stays.
    kit.adopt_as(&ana, ws, "ana-sub", Some(id.clone()), false)
        .await
        .unwrap();
    let view = kit
        .get(&ana, &format!("/api/v1/w/{ws}/connections/{id}"))
        .await;
    assert_eq!(view.json["grantor_signs_in"], true);
}
