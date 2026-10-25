//! Who sees which connection.

use axum::http::StatusCode;
use campbooks_kernel::tenancy::{Role, WorkspaceId};
use sqlx::PgPool;

use crate::{config::ConnectorsConfig, domain::model::Provider, testkit::Kit};

#[sqlx::test(migrations = "../../../migrations")]
async fn grantors_admins_and_clients_see_a_connection(db: PgPool) {
    let kit = Kit::new(db).await;
    let ws = WorkspaceId::new();
    let ana = kit.user("ana");
    kit.join(&ana, ws, Role::Member);
    let bea = kit.user("bea");
    kit.join(&bea, ws, Role::Member);
    let carl = kit.user("carl");
    kit.join(&carl, ws, Role::Admin);
    let id = kit.connect(&ana, ws).await["id"]
        .as_str()
        .unwrap()
        .to_owned();

    let list = |caller: String| {
        let kit = &kit;
        async move {
            kit.get(&caller, &format!("/api/v1/w/{ws}/connections"))
                .await
                .json["connections"]
                .as_array()
                .unwrap()
                .len()
        }
    };
    assert_eq!(list(ana.token.clone()).await, 1);
    assert_eq!(list(bea.token.clone()).await, 0);
    assert_eq!(list(carl.token.clone()).await, 1);
    assert_eq!(list(kit.client(ws).token).await, 1);

    let one = format!("/api/v1/w/{ws}/connections/{id}");
    assert_eq!(kit.get(&ana, &one).await.json["granted_by_you"], true);
    assert_eq!(kit.get(&carl, &one).await.json["granted_by_you"], false);
    assert_eq!(
        kit.get(&kit.client(ws).token, &one).await.json["granted_by_you"],
        false
    );
    assert_eq!(kit.get(&ana, &one).await.status, StatusCode::OK);
    assert_eq!(kit.get(&bea, &one).await.status, StatusCode::NOT_FOUND);
    assert_eq!(kit.get(&carl, &one).await.status, StatusCode::OK);
    // Another workspace's caller, or another workspace's path: 404.
    let other = WorkspaceId::new();
    kit.join(&bea, other, Role::Owner);
    assert_eq!(
        kit.get(&bea, &format!("/api/v1/w/{other}/connections/{id}"))
            .await
            .status,
        StatusCode::NOT_FOUND
    );
    assert_eq!(kit.get("", &one).await.status, StatusCode::UNAUTHORIZED);
}

/// The `offers` of `connection`, as the API lists them.
fn offers(connection: &serde_json::Value) -> Vec<&str> {
    connection["offers"]
        .as_array()
        .expect("offers")
        .iter()
        .map(|each| each.as_str().unwrap())
        .collect()
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_connection_offers_what_it_could_still_be_given(db: PgPool) {
    let kit = Kit::new(db).await;
    let ws = WorkspaceId::new();
    let ana = kit.user("ana");
    kit.join(&ana, ws, Role::Member);

    // Google: calendar access, until it is held (a mailbox-only consent).
    let google = kit.connect(&ana, ws).await;
    assert!(
        google["capabilities"]
            .as_array()
            .unwrap()
            .iter()
            .all(|each| each != "calendar")
    );
    assert_eq!(offers(&google), ["calendar"]);
    let one = format!(
        "/api/v1/w/{ws}/connections/{}",
        google["id"].as_str().unwrap()
    );
    assert_eq!(offers(&kit.get(&ana, &one).await.json), ["calendar"]);

    // Zoho: nothing while its calendar adapter is off.
    let zoho = kit.connect_zoho(&ana, ws).await;
    assert!(offers(&zoho).is_empty(), "{zoho}");
    let list = kit.get(&ana, &format!("/api/v1/w/{ws}/connections")).await;
    assert!(
        list.json["connections"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|each| each["provider"] == "zoho")
            .all(|each| offers(each).is_empty())
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn zoho_offers_calendar_only_while_its_flag_is_on(db: PgPool) {
    let mut config = ConnectorsConfig::default();
    config.zoho.requests_per_minute = 1_000_000;
    config.imap.enabled = false;
    config.zoho_calendar = true;
    let kit = Kit::with_config(db, config).await;
    let ws = WorkspaceId::new();
    let ana = kit.user("ana");
    kit.join(&ana, ws, Role::Member);

    let zoho = kit.connect_zoho(&ana, ws).await;
    // The test's consent grants mail only, so calendar is on offer.
    assert_eq!(offers(&zoho), ["calendar"], "{zoho}");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn microsoft_and_imap_offer_nothing(db: PgPool) {
    let kit = Kit::with_microsoft(db).await;
    let ws = WorkspaceId::new();
    let ana = kit.user("ana");
    kit.join(&ana, ws, Role::Member);
    let microsoft = kit.connect_microsoft(&ana, ws).await;
    assert!(offers(&microsoft).is_empty(), "{microsoft}");
    for provider in [Provider::Microsoft, Provider::Imap] {
        assert!(kit.deps.offers(provider, &[]).is_empty());
    }
}
