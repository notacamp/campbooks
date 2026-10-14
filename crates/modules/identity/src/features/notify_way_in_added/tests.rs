//! The notice of a new way in: who gets it, in which language, what it
//! says (and does not), once, and what happens without a mail server.

use std::sync::Arc;

use campbooks_kernel::{
    auth::Principal,
    i18n::{Locale, LocaleFuture, LocalePreference},
    jobs::Worker,
    mail::NoMailer,
};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    events::WayInAdded,
    testkit::{Kit, ORIGIN},
    upstream::Provider,
    ways_in_tests::{link_to, user},
};

/// Everyone chose European Portuguese.
#[derive(Debug)]
struct Portuguese;

impl LocalePreference for Portuguese {
    fn preferred<'a>(&'a self, _db: &'a PgPool, _principal: &'a Principal) -> LocaleFuture<'a> {
        Box::pin(async { Ok(Some(Locale::PtPt)) })
    }
}

/// Sign up with Google, then add Zoho (`zoho-sam@example.com`): the account.
async fn link_zoho(kit: &Kit) -> Uuid {
    let access = kit.signed_up().await;
    let (_, me) = Kit::decode(&access);
    kit.zoho.set_user(user("zoho-sam"));
    let at_zoho = link_to(kit, &access, Provider::Zoho).await;
    kit.via(Provider::Zoho, &at_zoho).await;
    me["sub"].as_str().unwrap().parse().unwrap()
}

async fn audits(kit: &Kit, account: Uuid) -> Vec<serde_json::Value> {
    sqlx::query_scalar(
        "SELECT metadata FROM identity.audit_events
          WHERE account_id = $1 AND kind = 'way_in_notice' ORDER BY at",
    )
    .bind(account)
    .fetch_all(&kit.state.db)
    .await
    .unwrap()
}

async fn primary(kit: &Kit, account: Uuid) -> String {
    sqlx::query_scalar("SELECT primary_email FROM identity.accounts WHERE id = $1")
        .bind(account)
        .fetch_one(&kit.state.db)
        .await
        .unwrap()
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_linked_provider_is_announced_to_both_addresses_once(db: PgPool) {
    let kit = Kit::new(db).await;
    let account = link_zoho(&kit).await;
    kit.run_jobs().await;

    let sent = kit.outbox.sent();
    let mut to: Vec<&str> = sent.iter().map(|m| m.to.as_str()).collect();
    to.sort_unstable();
    let primary = primary(&kit, account).await;
    let mut expected = vec![primary.as_str(), "zoho-sam@example.com"];
    expected.sort_unstable();
    assert_eq!(to, expected);
    let mail = &sent[0];
    assert_eq!(
        mail.subject,
        "A new way to sign in to your Campbooks account"
    );
    assert!(
        mail.text.contains("A Zoho account was added"),
        "{}",
        mail.text
    );
    assert!(mail.text.contains(" UTC."), "{}", mail.text);
    assert!(mail.text.contains(ORIGIN), "{}", mail.text);
    assert!(
        mail.html.contains("A Zoho account was added"),
        "{}",
        mail.html
    );
    // No images, no remote content, one link: to this instance.
    assert!(!mail.html.contains("<img"), "{}", mail.html);
    assert_eq!(mail.html.matches("href=").count(), 1, "{}", mail.html);
    assert!(
        mail.html.contains(&format!("href=\"{ORIGIN}\"")),
        "{}",
        mail.html
    );
    assert!(!mail.html.contains("src="), "{}", mail.html);
    let audit = audits(&kit, account).await;
    assert_eq!(audit.len(), 1);
    assert_eq!(audit[0]["sent"], 2);
    // The event and the audit name no address.
    let payload: serde_json::Value = sqlx::query_scalar(
        "SELECT payload FROM kernel.events WHERE name = 'identity.way_in_added'",
    )
    .fetch_one(&kit.state.db)
    .await
    .unwrap();
    assert!(!payload.to_string().contains('@'), "{payload}");
    assert!(payload["way_in"].is_string(), "{payload}");
    assert!(!audit[0].to_string().contains('@'));

    // Delivered again (at least once): nothing more is sent.
    sqlx::query(
        "UPDATE kernel.jobs SET state = 'pending', run_at = now(), attempts = 0, finished_at = NULL
          WHERE kind = 'identity.notify_way_in_added'",
    )
    .execute(&kit.state.db)
    .await
    .unwrap();
    kit.run_jobs().await;
    assert_eq!(kit.outbox.sent().len(), 2);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_notice_speaks_the_persons_language(db: PgPool) {
    let kit = Kit::new(db).await;
    link_zoho(&kit).await;
    Worker::new(kit.state.clone().with_locales(Arc::new(Portuguese)))
        .run_until_idle()
        .await;
    let sent = kit.outbox.sent();
    assert_eq!(sent.len(), 2);
    assert_eq!(
        sent[0].subject,
        "Uma nova forma de iniciar sessão na sua conta Campbooks"
    );
    assert!(
        sent[0].text.contains("Uma conta Zoho foi adicionada"),
        "{}",
        sent[0].text
    );
    assert!(sent[0].html.contains("lang=\"pt-PT\""), "{}", sent[0].html);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_unreachable_mail_server_is_tried_again(db: PgPool) {
    let kit = Kit::new(db).await;
    let account = link_zoho(&kit).await;
    kit.outbox.fail(true);
    kit.run_jobs().await;
    assert!(kit.outbox.sent().is_empty());
    assert!(audits(&kit, account).await.is_empty(), "not done yet");
    let waiting: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM kernel.jobs
          WHERE kind = 'identity.notify_way_in_added' AND attempts = 1 AND last_error IS NOT NULL",
    )
    .fetch_one(&kit.state.db)
    .await
    .unwrap();
    assert_eq!(waiting, 1);
    let error: String = sqlx::query_scalar(
        "SELECT last_error FROM kernel.jobs WHERE kind = 'identity.notify_way_in_added'",
    )
    .fetch_one(&kit.state.db)
    .await
    .unwrap();
    assert!(!error.contains('@'), "{error}");

    kit.outbox.fail(false);
    sqlx::query(
        "UPDATE kernel.jobs SET run_at = now() WHERE kind = 'identity.notify_way_in_added'",
    )
    .execute(&kit.state.db)
    .await
    .unwrap();
    kit.run_jobs().await;
    assert_eq!(kit.outbox.sent().len(), 2);
    assert_eq!(audits(&kit, account).await[0]["sent"], 2);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn without_a_mail_server_the_notice_is_kept_in_the_account(db: PgPool) {
    let kit = Kit::new(db).await;
    let account = link_zoho(&kit).await;
    Worker::new(kit.state.clone().with_mailer(Arc::new(NoMailer)))
        .run_until_idle()
        .await;
    assert!(kit.outbox.sent().is_empty());
    let audit = audits(&kit, account).await;
    assert_eq!(audit.len(), 1);
    assert_eq!(audit[0]["sent"], 0);
    assert_eq!(audit[0]["reason"], "no_mail_transport");
    // Ways to sign in still says when it was added (the app highlights it).
    let ways: Vec<chrono::DateTime<chrono::Utc>> = sqlx::query_scalar(
        "SELECT linked_at FROM identity.provider_identities WHERE account_id = $1",
    )
    .bind(account)
    .fetch_all(&kit.state.db)
    .await
    .unwrap();
    assert_eq!(ways.len(), 2);
}

/// Publish a way-in event for `account` as the module would.
async fn publish(kit: &Kit, account: Uuid, via: &str, way_in: Option<Uuid>) {
    let mut tx = kit.state.db.begin().await.unwrap();
    kit.state
        .events
        .publish(
            &mut tx,
            &WayInAdded {
                account,
                method: "passkey".to_owned(),
                provider: None,
                via: via.to_owned(),
                way_in,
            },
        )
        .await
        .unwrap();
    tx.commit().await.unwrap();
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_passkey_is_announced_to_the_verified_primary_address_only(db: PgPool) {
    let kit = Kit::new(db).await;
    let access = kit.signed_up().await;
    let (_, me) = Kit::decode(&access);
    let account: Uuid = me["sub"].as_str().unwrap().parse().unwrap();
    // A sign-up's first way in: no notice.
    publish(&kit, account, "sign_up", Some(Uuid::now_v7())).await;
    kit.run_jobs().await;
    assert!(kit.outbox.sent().is_empty());

    publish(&kit, account, "passkey", Some(Uuid::now_v7())).await;
    kit.run_jobs().await;
    let sent = kit.outbox.sent();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].to, primary(&kit, account).await);
    assert!(
        sent[0].text.starts_with("A passkey was added"),
        "{}",
        sent[0].text
    );

    // An address nobody verified gets nothing; nor does a disabled account.
    sqlx::query("UPDATE identity.accounts SET primary_email_verified = false WHERE id = $1")
        .bind(account)
        .execute(&kit.state.db)
        .await
        .unwrap();
    publish(&kit, account, "passkey", Some(Uuid::now_v7())).await;
    kit.run_jobs().await;
    assert_eq!(kit.outbox.sent().len(), 1);
    sqlx::query(
        "UPDATE identity.accounts SET primary_email_verified = true, disabled_at = now()
          WHERE id = $1",
    )
    .bind(account)
    .execute(&kit.state.db)
    .await
    .unwrap();
    publish(&kit, account, "passkey", Some(Uuid::now_v7())).await;
    kit.run_jobs().await;
    assert_eq!(kit.outbox.sent().len(), 1);
}
