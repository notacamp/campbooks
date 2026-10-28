use axum::http::StatusCode;
use campbooks_kernel::tenancy::WorkspaceId;
use serde_json::{Value, json};
use sqlx::PgPool;

use crate::{
    model::InboxRole,
    testkit::{Kit, SAM, mail},
};

const MAYA: &str = "maya@northwind-studio.example";
const LEO: &str = "leo@northwind-studio.example";
const DANA: (&str, &str) = ("Dana Whitfield", "dana@harbour-books.example");

/// [`mail`] sent by `from`, an inbox other than Sam's, to `to`.
fn sent_by(id: &str, from: (&str, &str), to: &[(&str, &str)]) -> Value {
    let mut sent = mail(id, id, from, to, &[], 30);
    sent["changes"][0]["message"]["direction"] = json!("outbound");
    sent
}

/// What `list_correspondents` answers `token` for `q` (already URL-encoded).
async fn found(kit: &Kit, token: &str, workspace: WorkspaceId, q: &str) -> Vec<Value> {
    let response = kit
        .get(
            token,
            &format!("/api/v1/w/{workspace}/correspondents?q={q}"),
        )
        .await;
    assert_eq!(response.status, StatusCode::OK, "{}", response.json);
    response.json["correspondents"].as_array().unwrap().clone()
}

/// Just the addresses of [`found`].
async fn addresses(kit: &Kit, token: &str, workspace: WorkspaceId, q: &str) -> Vec<String> {
    found(kit, token, workspace, q)
        .await
        .iter()
        .map(|person| person["address"].as_str().unwrap().to_owned())
        .collect()
}

#[sqlx::test(migrations = "../../../migrations")]
async fn by_name_or_domain_ignoring_case_and_accents(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, _, sam) = kit.sams_inbox().await;

    // Maya wrote once and Sam replied once (fixtures/en.json).
    let by_name = found(&kit, &sam, workspace, "Ok%C3%A1for").await;
    assert_eq!(by_name.len(), 1);
    assert_eq!(by_name[0]["address"], MAYA);
    assert_eq!(by_name[0]["name"], "Maya Okafor");
    assert_eq!(by_name[0]["wrote"], 1);
    assert_eq!(by_name[0]["received"], 1);
    let mut keys: Vec<_> = by_name[0].as_object().unwrap().keys().cloned().collect();
    keys.sort();
    assert_eq!(keys, ["address", "name", "received", "wrote"]);

    // Leo was copied on Maya's email: listed after Maya, who wrote once.
    assert_eq!(
        addresses(&kit, &sam, workspace, "NORTHWIND").await,
        [MAYA, LEO]
    );
    assert_eq!(
        addresses(&kit, &sam, workspace, "paperkite").await,
        ["hello@letters.paperkite.example"]
    );
    assert!(
        addresses(&kit, &sam, workspace, "maya%20cloudvault")
            .await
            .is_empty()
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn someone_only_written_to_is_suggested(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, inbox, sam) = kit.sams_inbox().await;
    // Written to with a capitalised address, then in both To and Cc of one
    // message: two messages, not three.
    let shouting = ("Dana Whitfield", "Dana@Harbour-Books.example");
    kit.ingest_value(
        workspace,
        inbox,
        &mail("dana-1", "thr-dana", SAM, &[shouting], &[], 90),
    )
    .await;
    kit.ingest_value(
        workspace,
        inbox,
        &mail("dana-2", "thr-dana", SAM, &[DANA], &[DANA], 30),
    )
    .await;

    let dana = found(&kit, &sam, workspace, "whitf").await;
    assert_eq!(dana.len(), 1);
    assert_eq!(dana[0]["address"], DANA.1);
    assert_eq!(dana[0]["name"], DANA.0);
    assert_eq!(dana[0]["wrote"], 2);
    assert_eq!(dana[0]["received"], 0);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn written_to_most_first(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, inbox, sam) = kit.sams_inbox().await;
    let rae = ("Rae Rivera", "rae@rivera-design.example");
    let rui = ("Rui Rivera", "rui@rivera-law.example");
    for (id, minutes_ago) in [("rae-1", 300), ("rae-2", 200)] {
        kit.ingest_value(
            workspace,
            inbox,
            &mail(id, id, SAM, &[rae], &[], minutes_ago),
        )
        .await;
    }
    for (id, minutes_ago) in [("rui-1", 290), ("rui-2", 190), ("rui-3", 90)] {
        kit.ingest_value(
            workspace,
            inbox,
            &mail(id, id, rui, &[SAM], &[], minutes_ago),
        )
        .await;
    }

    let riveras = found(&kit, &sam, workspace, "rivera").await;
    let ranked: Vec<_> = riveras
        .iter()
        .map(|p| {
            (
                p["address"].as_str().unwrap(),
                p["wrote"].clone(),
                p["received"].clone(),
            )
        })
        .collect();
    assert_eq!(
        ranked,
        [(rae.1, json!(2), json!(0)), (rui.1, json!(0), json!(3))]
    );
    let one = kit
        .get(
            &sam,
            &format!("/api/v1/w/{workspace}/correspondents?q=rivera&limit=1"),
        )
        .await;
    assert_eq!(one.json["correspondents"][0]["address"], rae.1);
    assert_eq!(one.json["correspondents"].as_array().unwrap().len(), 1);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn not_someone_to_write_to_unless_written_to(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, inbox, sam) = kit.sams_inbox().await;
    // The phishing sender under review, a no-reply address, and a sender
    // with one conversation reported and another left alone.
    kit.ingest(workspace, inbox, "needs").await;
    let phish = "it-support@at3lier-sol.example";
    let no_reply = ("Accounts", "no-reply@accounts.example");
    let lumen = ("Lumen Billing", "billing@lumen-pay.example");
    kit.ingest_value(
        workspace,
        inbox,
        &mail("accounts-1", "thr-accounts", no_reply, &[SAM], &[], 120),
    )
    .await;
    for (id, minutes_ago) in [("lumen-1", 100), ("lumen-2", 80)] {
        kit.ingest_value(
            workspace,
            inbox,
            &mail(id, id, lumen, &[SAM], &[], minutes_ago),
        )
        .await;
    }
    assert_eq!(addresses(&kit, &sam, workspace, "lumen").await, [lumen.1]);
    let reported = kit.conversation_of("lumen-1").await;
    let spam = kit
        .post(
            &sam,
            &format!("/api/v1/w/{workspace}/conversations/{reported}/spam"),
            None,
        )
        .await;
    assert_eq!(spam.status, StatusCode::OK);

    for q in ["support", "at3lier", "accounts", "no-reply", "lumen"] {
        let listed = addresses(&kit, &sam, workspace, q).await;
        for left_out in [
            phish,
            no_reply.1,
            lumen.1,
            "no-reply@accounts.cloudvault.example",
        ] {
            assert!(!listed.contains(&left_out.to_owned()), "{q}: {listed:?}");
        }
    }

    // Sam wrote to each of them: that was Sam's own choice.
    let phisher = ("IT Support", phish);
    for (id, to) in [
        ("to-phish", phisher),
        ("to-accounts", no_reply),
        ("to-lumen", lumen),
    ] {
        kit.ingest_value(workspace, inbox, &mail(id, id, SAM, &[to], &[], 10))
            .await;
    }
    assert_eq!(addresses(&kit, &sam, workspace, "at3lier").await, [phish]);
    assert!(
        addresses(&kit, &sam, workspace, "accounts")
            .await
            .contains(&no_reply.1.to_owned())
    );
    assert_eq!(addresses(&kit, &sam, workspace, "lumen").await, [lumen.1]);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn never_yourself(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, inbox, sam) = kit.sams_inbox().await;
    // Sam copied themself on a message to Dana.
    kit.ingest_value(
        workspace,
        inbox,
        &mail("dana-1", "thr-dana", SAM, &[DANA], &[SAM], 30),
    )
    .await;
    // And Maya's message to Sam had Sam in Cc as well as To.
    let maya = ("Maya Okafor", MAYA);
    kit.ingest_value(
        workspace,
        inbox,
        &mail("maya-2", "thr-maya-2", maya, &[SAM], &[SAM], 20),
    )
    .await;
    assert!(addresses(&kit, &sam, workspace, "sam").await.is_empty());
    assert!(
        addresses(&kit, &sam, workspace, "example.com")
            .await
            .iter()
            .all(|address| address != SAM.1)
    );
    assert_eq!(addresses(&kit, &sam, workspace, "dana").await, [DANA.1]);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn someone_copied_is_suggested(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, inbox, sam) = kit.sams_inbox().await;

    // Leo is in Cc of Maya's email (fixtures/en.json); Sam never wrote to him.
    let leo = found(&kit, &sam, workspace, "leo").await;
    assert_eq!(leo.len(), 1);
    assert_eq!(leo[0]["address"], LEO);
    assert_eq!(leo[0]["name"], "Leo Park");
    assert_eq!(leo[0]["wrote"], 0);
    assert_eq!(leo[0]["received"], 0);

    // Someone in To alongside Sam counts too.
    let noor = ("Noor Haddad", "noor@haddad-works.example");
    kit.ingest_value(
        workspace,
        inbox,
        &mail(
            "noor-1",
            "thr-noor",
            ("Pia Costa", "pia@costa-works.example"),
            &[SAM, noor],
            &[],
            20,
        ),
    )
    .await;
    assert_eq!(addresses(&kit, &sam, workspace, "haddad").await, [noor.1]);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn copied_after_those_who_wrote(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, inbox, sam) = kit.sams_inbox().await;
    let rae = ("Rae Rivera", "rae@rivera-design.example");
    let rui = ("Rui Rivera", "rui@rivera-law.example");
    let ria = ("Ria Rivera", "ria@rivera-print.example");
    let rob = ("Rob Rivera", "rob@rivera-wood.example");
    let pia = ("Pia Costa", "pia@costa-works.example");
    kit.ingest_value(
        workspace,
        inbox,
        &mail("rae-1", "rae-1", SAM, &[rae], &[], 300),
    )
    .await;
    // Rui: copied on three messages, in Cc each time.
    for (id, minutes_ago) in [("rui-1", 290), ("rui-2", 190), ("rui-3", 90)] {
        kit.ingest_value(
            workspace,
            inbox,
            &mail(id, id, pia, &[SAM], &[rui], minutes_ago),
        )
        .await;
    }
    // Ria: copied on one message.
    kit.ingest_value(
        workspace,
        inbox,
        &mail("ria-1", "ria-1", pia, &[SAM], &[ria], 80),
    )
    .await;
    // Rob: in both To and Cc of one message, still once; Ria's one copy
    // and his one are tied, so the address orders them.
    kit.ingest_value(
        workspace,
        inbox,
        &mail("rob-1", "rob-1", pia, &[SAM, rob], &[rob], 70),
    )
    .await;

    let ranked: Vec<_> = found(&kit, &sam, workspace, "rivera")
        .await
        .iter()
        .map(|p| {
            (
                p["address"].as_str().unwrap().to_owned(),
                p["wrote"].clone(),
            )
        })
        .collect();
    assert_eq!(
        ranked,
        [
            (rae.1.to_owned(), json!(1)),
            (rui.1.to_owned(), json!(0)),
            (ria.1.to_owned(), json!(0)),
            (rob.1.to_owned(), json!(0)),
        ]
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn copied_by_someone_not_to_write_to(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, inbox, sam) = kit.sams_inbox().await;
    let helpdesk = ("Helpdesk", "helpdesk@at3lier-sol.example");
    let fine = ("Pia Costa", "pia@costa-works.example");

    // (a) A sender under review copies the helpdesk.
    kit.ingest(workspace, inbox, "needs").await;
    kit.ingest_value(
        workspace,
        inbox,
        &mail(
            "phish-2",
            "phish-2",
            ("IT Support", "it-support@at3lier-sol.example"),
            &[SAM],
            &[helpdesk],
            15,
        ),
    )
    .await;
    // (b) A fine sender copies a no-reply address.
    let robot = ("Robot", "no-reply@robot-mail.example");
    kit.ingest_value(
        workspace,
        inbox,
        &mail("pia-1", "pia-1", fine, &[SAM], &[robot], 14),
    )
    .await;
    // (c) A sender Sam reported copies someone.
    let shady = ("Shady Sales", "sales@shady-deals.example");
    let cc = ("Cora Quinn", "cora@quinn-works.example");
    kit.ingest_value(
        workspace,
        inbox,
        &mail("shady-1", "shady-1", shady, &[SAM], &[cc], 13),
    )
    .await;
    let reported = kit.conversation_of("shady-1").await;
    let spam = kit
        .post(
            &sam,
            &format!("/api/v1/w/{workspace}/conversations/{reported}/spam"),
            None,
        )
        .await;
    assert_eq!(spam.status, StatusCode::OK);
    // (d) A sender whose mail failed its checks copies someone.
    let spoof = ("Spoofer", "boss@spoof-corp.example");
    let dee = ("Dee Ward", "dee@ward-works.example");
    let mut failed = mail("spoof-1", "spoof-1", spoof, &[SAM], &[dee], 12);
    failed["changes"][0]["message"]["headers"] = json!({"authentication_results":
        "mx.example; dkim=fail header.i=@spoof-corp.example; dmarc=fail header.from=spoof-corp.example"});
    kit.ingest_value(workspace, inbox, &failed).await;

    for (q, left_out) in [
        ("helpdesk", helpdesk.1),
        ("robot", robot.1),
        ("cora", cc.1),
        ("dee", dee.1),
    ] {
        let listed = addresses(&kit, &sam, workspace, q).await;
        assert!(!listed.contains(&left_out.to_owned()), "{q}: {listed:?}");
    }

    // (e) Sam writes to the helpdesk: that was Sam's own choice.
    kit.ingest_value(
        workspace,
        inbox,
        &mail("to-helpdesk", "to-helpdesk", SAM, &[helpdesk], &[], 5),
    )
    .await;
    assert_eq!(
        addresses(&kit, &sam, workspace, "helpdesk").await,
        [helpdesk.1]
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn wildcards_match_themselves(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, inbox, sam) = kit.sams_inbox().await;
    let odd = ("50%_off Deals", "deals@odd.example");
    kit.ingest_value(
        workspace,
        inbox,
        &mail("odd-1", "odd-1", SAM, &[odd], &[], 30),
    )
    .await;
    // `%_` would match everyone as a pattern, and `a_a` Maya: as letters,
    // only a name that holds them.
    assert_eq!(addresses(&kit, &sam, workspace, "%25_").await, [odd.1]);
    assert!(addresses(&kit, &sam, workspace, "a_a").await.is_empty());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn bounded(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, _, sam) = kit.sams_inbox().await;
    let uri = format!("/api/v1/w/{workspace}/correspondents");
    let long = "a".repeat(101);

    for bad in [
        "?q=a".to_owned(),
        "?q=%20%20".to_owned(),
        "?q=%2C%2C".to_owned(),
        format!("?q={long}"),
        "?q=maya&limit=11".to_owned(),
        "?q=maya&limit=0".to_owned(),
    ] {
        let response = kit.get(&sam, &format!("{uri}{bad}")).await;
        assert_eq!(response.status, StatusCode::BAD_REQUEST, "{bad}");
        assert_eq!(
            response.json["code"], "invalid_correspondents_query",
            "{bad}"
        );
    }
    let missing = kit.get(&sam, &uri).await;
    assert_eq!(missing.status, StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn only_inboxes_the_caller_reads(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, inbox, _) = kit.sams_inbox().await;
    let (ana, ana_id) = kit.member("ana", workspace);
    let uri = format!("/api/v1/w/{workspace}/correspondents?q=maya");

    // Ana's own inbox, where she wrote to Dana: Sam's correspondents aren't hers.
    let anas = kit
        .inbox(
            workspace,
            "ana@example.com",
            &[(ana_id, InboxRole::Manager)],
        )
        .await;
    kit.ingest_value(
        workspace,
        anas,
        &sent_by("ana-dana", ("Ana", "ana@example.com"), &[DANA]),
    )
    .await;
    assert!(found(&kit, &ana, workspace, "maya").await.is_empty());
    assert_eq!(addresses(&kit, &ana, workspace, "dana").await, [DANA.1]);

    kit.add_to_inbox(inbox, ana_id, InboxRole::Reader).await;
    assert_eq!(addresses(&kit, &ana, workspace, "maya").await, [MAYA]);

    let stranger = kit
        .get(
            &ana,
            &format!("/api/v1/w/{}/correspondents?q=maya", WorkspaceId::new()),
        )
        .await;
    assert_eq!(stranger.status, StatusCode::NOT_FOUND);
    assert_eq!(kit.get("", &uri).await.status, StatusCode::UNAUTHORIZED);
}
