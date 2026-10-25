//! Finding a mailbox's servers: the bundled database first, then the
//! domain's own autoconfig, SRV and MX, and native providers routed to
//! their own sign-in. No test reaches a real resolver or web server.

use axum::http::StatusCode;
use campbooks_kernel::tenancy::{Role, WorkspaceId};
use sqlx::PgPool;

use crate::{testkit::Kit, testkit_imap};

fn discover_uri(ws: WorkspaceId) -> String {
    format!("/api/v1/w/{ws}/connections/imap/discover")
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_big_provider_comes_from_the_bundled_database(db: PgPool) {
    let (kit, fakes) = testkit_imap::kit(db).await;
    let ws = WorkspaceId::new();
    let ana = kit.user_signed_in("ana", 2);
    kit.join(&ana, ws, Role::Member);

    let found = kit
        .post(
            &ana,
            &discover_uri(ws),
            serde_json::json!({"address": "Ana@Yahoo.com"}),
        )
        .await;
    assert_eq!(found.status, StatusCode::OK, "{:?}", found.json);
    assert_eq!(found.json["kind"], "settings");
    assert_eq!(
        found.json["imap"],
        serde_json::json!({"host": "imap.mail.yahoo.com", "port": 993, "security": "tls"})
    );
    assert_eq!(found.json["smtp"]["host"], "smtp.mail.yahoo.com");
    assert_eq!(found.json["username_hint"], "ana@yahoo.com");
    assert_eq!(found.json["source"], "ispdb");
    assert_eq!(found.json["provider_hint"], "yahoo.com");
    assert_eq!(found.json["password"], "app_password");
    // Nothing was asked of anyone: not the web, not DNS.
    assert!(fakes.web.asked().is_empty());
    assert!(fakes.dns.asked().is_empty());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_custom_domain_publishes_its_own_settings(db: PgPool) {
    let (kit, fakes) = testkit_imap::kit(db).await;
    let ws = WorkspaceId::new();
    let ana = kit.user_signed_in("ana", 2);
    kit.join(&ana, ws, Role::Member);
    fakes.web.serve(
        "https://autoconfig.bakery.example/mail/config-v1.1.xml",
        r#"<?xml version="1.0"?>
        <clientConfig version="1.1">
          <emailProvider id="bakery.example">
            <domain>bakery.example</domain>
            <incomingServer type="imap">
              <hostname>mail.bakery.example</hostname><port>993</port>
              <socketType>SSL</socketType><username>%EMAILADDRESS%</username>
              <authentication>password-cleartext</authentication>
            </incomingServer>
            <outgoingServer type="smtp">
              <hostname>mail.bakery.example</hostname><port>587</port>
              <socketType>STARTTLS</socketType><username>%EMAILADDRESS%</username>
              <authentication>password-cleartext</authentication>
            </outgoingServer>
          </emailProvider>
        </clientConfig>"#,
    );

    let found = kit
        .post(
            &ana,
            &discover_uri(ws),
            serde_json::json!({"address": "ana@bakery.example"}),
        )
        .await;
    assert_eq!(found.status, StatusCode::OK, "{:?}", found.json);
    assert_eq!(found.json["kind"], "settings");
    assert_eq!(found.json["source"], "autoconfig");
    assert_eq!(found.json["imap"]["host"], "mail.bakery.example");
    assert_eq!(
        found.json["smtp"],
        serde_json::json!({"host": "mail.bakery.example", "port": 587, "security": "starttls"})
    );
    assert_eq!(found.json["password"], "unknown");
    assert!(found.json.get("provider_hint").is_none());
    // Only the domain's own server was asked, and never with the address.
    for url in fakes.web.asked() {
        assert!(
            url.starts_with("https://autoconfig.bakery.example/"),
            "{url}"
        );
        assert!(!url.contains("ana"), "{url}");
    }
}

#[sqlx::test(migrations = "../../../migrations")]
async fn srv_records_name_the_servers(db: PgPool) {
    let (kit, fakes) = testkit_imap::kit(db).await;
    let ws = WorkspaceId::new();
    let ana = kit.user_signed_in("ana", 2);
    kit.join(&ana, ws, Role::Member);
    fakes
        .dns
        .set_srv("_imaps._tcp.shop.example", "imap.shop.example", 993);
    fakes
        .dns
        .set_srv("_submission._tcp.shop.example", "smtp.shop.example", 587);

    let found = kit
        .post(
            &ana,
            &discover_uri(ws),
            serde_json::json!({"address": "ana@shop.example"}),
        )
        .await;
    assert_eq!(found.json["kind"], "settings", "{:?}", found.json);
    assert_eq!(found.json["source"], "srv");
    assert_eq!(
        found.json["imap"],
        serde_json::json!({"host": "imap.shop.example", "port": 993, "security": "tls"})
    );
    assert_eq!(
        found.json["smtp"],
        serde_json::json!({"host": "smtp.shop.example", "port": 587, "security": "starttls"})
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_custom_domain_on_a_native_provider_is_sent_there(db: PgPool) {
    let (kit, fakes) = testkit_imap::kit(db).await;
    let ws = WorkspaceId::new();
    let ana = kit.user_signed_in("ana", 2);
    kit.join(&ana, ws, Role::Member);
    fakes.dns.set_mx("bakery.example", &["aspmx.l.google.com"]);
    fakes.dns.set_mx(
        "dairy.example",
        &["dairy-example.mail.protection.outlook.com"],
    );

    let google = kit
        .post(
            &ana,
            &discover_uri(ws),
            serde_json::json!({"address": "ana@bakery.example"}),
        )
        .await;
    assert_eq!(google.status, StatusCode::OK);
    assert_eq!(
        google.json,
        serde_json::json!({"kind": "use_provider", "provider": "google"})
    );
    let gmail = kit
        .post(
            &ana,
            &discover_uri(ws),
            serde_json::json!({"address": "ana@gmail.com"}),
        )
        .await;
    assert_eq!(gmail.json["provider"], "google");
    let microsoft = kit
        .post(
            &ana,
            &discover_uri(ws),
            serde_json::json!({"address": "ana@dairy.example"}),
        )
        .await;
    assert_eq!(
        microsoft.json,
        serde_json::json!({"kind": "not_supported_yet", "provider": "microsoft"})
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn nothing_found_asks_for_the_servers(db: PgPool) {
    let (kit, _fakes) = testkit_imap::kit(db).await;
    let ws = WorkspaceId::new();
    let ana = kit.user_signed_in("ana", 2);
    kit.join(&ana, ws, Role::Member);

    let found = kit
        .post(
            &ana,
            &discover_uri(ws),
            serde_json::json!({"address": "ana@nowhere.example"}),
        )
        .await;
    assert_eq!(found.status, StatusCode::OK);
    assert_eq!(found.json, serde_json::json!({"kind": "not_found"}));
    let bad = kit
        .post(
            &ana,
            &discover_uri(ws),
            serde_json::json!({"address": "not an address"}),
        )
        .await;
    assert_eq!(bad.status, StatusCode::BAD_REQUEST);
    assert_eq!(bad.json["code"], "imap_address_invalid");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn only_a_recently_signed_in_member_on_an_instance_with_imap(db: PgPool) {
    let (kit, _fakes) = testkit_imap::kit(db.clone()).await;
    let ws = WorkspaceId::new();
    let body = serde_json::json!({"address": "ana@yahoo.com"});
    let stale = kit.user_signed_in("ana", 11);
    kit.join(&stale, ws, Role::Member);
    let refused = kit.post(&stale, &discover_uri(ws), body.clone()).await;
    assert_eq!(refused.status, StatusCode::FORBIDDEN);
    assert_eq!(refused.json["code"], "step_up_required");
    let client = kit.client(ws);
    let refused = kit.post(&client, &discover_uri(ws), body.clone()).await;
    assert_eq!(refused.status, StatusCode::FORBIDDEN);
    assert_eq!(refused.json["code"], "connection_needs_user");

    // Turned off (`Kit::new`): not offered.
    let off = Kit::new(db).await;
    let bea = off.user_signed_in("bea", 2);
    off.join(&bea, ws, Role::Member);
    let refused = off.post(&bea, &discover_uri(ws), body).await;
    assert_eq!(refused.status, StatusCode::CONFLICT);
    assert_eq!(refused.json["code"], "provider_not_configured");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn discovery_is_limited_per_member(db: PgPool) {
    let (kit, _fakes) = testkit_imap::kit(db).await;
    let ws = WorkspaceId::new();
    let ana = kit.user_signed_in("ana", 2);
    kit.join(&ana, ws, Role::Member);
    let body = serde_json::json!({"address": "ana@yahoo.com"});
    for _ in 0..super::DISCOVERIES_PER_HOUR {
        let found = kit.post(&ana, &discover_uri(ws), body.clone()).await;
        assert_eq!(found.status, StatusCode::OK);
    }
    let limited = kit.post(&ana, &discover_uri(ws), body).await;
    assert_eq!(limited.status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(limited.json["code"], "rate_limited");
}

#[test]
fn outlook_goes_to_microsofts_sign_in_only_while_it_is_offered() {
    use std::sync::Arc;

    use super::{DiscoveredImap, offered};
    use crate::imap::discover::{Discovery, Native};

    let outlook = || Discovery::NotSupportedYet(Native::Microsoft);
    // Off (the default, or the client missing): coming soon.
    let off = crate::Deps::default();
    assert_eq!(offered(&off, outlook()), outlook());
    assert_eq!(
        offered(&off, Discovery::UseProvider(Native::Microsoft)),
        outlook()
    );
    // Offered: Microsoft's own sign-in, like Gmail.
    let on = crate::Deps {
        microsoft: Some(Arc::new(
            crate::Microsoft::new(
                "microsoft-client-1".into(),
                crate::ClientSecret::Inline("s3cret".into()),
                crate::microsoft::Endpoints::microsoft("common").unwrap(),
            )
            .unwrap(),
        )),
        ..crate::Deps::default()
    };
    let routed = offered(&on, outlook());
    assert_eq!(routed, Discovery::UseProvider(Native::Microsoft));
    assert_eq!(
        serde_json::to_value(DiscoveredImap::from(routed)).unwrap(),
        serde_json::json!({"kind": "use_provider", "provider": "microsoft"})
    );
    // Nothing else changes.
    for other in [
        Discovery::UseProvider(Native::Google),
        Discovery::UseProvider(Native::Zoho),
        Discovery::NotFound,
    ] {
        assert_eq!(offered(&on, other.clone()), other);
        assert_eq!(offered(&off, other.clone()), other);
    }
}
