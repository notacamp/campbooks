//! Tests of `search.query` over a fake source (fictional mail only).

use std::{fmt::Write as _, sync::atomic::Ordering};

use campbooks_kernel::tenancy::{UserId, WorkspaceId};
use sqlx::PgPool;

use crate::testkit::{Conversation, Kit, Message};

/// A workspace whose inbox `sam` (Sam's member token) reads, holding mail in
/// four languages.
async fn mailbox(kit: &Kit) -> (WorkspaceId, String, UserId) {
    let workspace = WorkspaceId::new();
    let (token, sam) = kit.member("sam", workspace);
    kit.share(workspace, "inbox-sam", &[sam]);
    let conversations = [
        (
            "c-water",
            Conversation::new(
                "inbox-sam",
                "A sua fatura de setembro está disponível",
                vec![
                    Message::new(
                        "m-water",
                        "Águas do Vale <faturacao@aguas-do-vale.example>",
                        "Olá Inês, as faturas de João e a fatura n.º FT 2026/09812, no valor de 23,47 €, já estão disponíveis.",
                        12,
                    )
                    .with_files(&["fatura-2026-09812.pdf"]),
                ],
            ),
        ),
        (
            "c-q4",
            Conversation::new(
                "inbox-sam",
                "Q4 plan: can you review by Friday?",
                vec![
                    Message::new(
                        "m-q4-1",
                        "Ana Lopes <ana.lopes@northwind.example>",
                        "Hi Sam, I've attached the Q4 plan. Could you review the budget section and the boiler repair invoice?",
                        3,
                    )
                    .with_files(&["q4-plan.pdf"]),
                    Message::new(
                        "m-q4-2",
                        "Sam Rivera <sam@example.com>",
                        "Will do, you'll have my notes on Thursday.",
                        2,
                    ),
                ],
            ),
        ),
        (
            "c-factura",
            Conversation::new(
                "inbox-sam",
                "Factura F-2026-0457 – pedido de septiembre",
                vec![Message::new(
                    "m-factura",
                    "administracion@harinas-del-sur.example",
                    "Buenos días, adjuntamos la factura F-2026-0457 por importe de 1.284,60 €.",
                    20,
                )],
            ),
        ),
        (
            "c-fuite",
            Conversation::new(
                "inbox-sam",
                "Fuite d'eau dans la cave",
                vec![Message::new(
                    "m-fuite",
                    "Hugo Bernard <hugo.bernard@courriel.example>",
                    "Bonsoir Camille, il y a une fuite d'eau dans la cave près de votre box.",
                    1,
                )],
            ),
        ),
    ];
    for (id, conversation) in conversations {
        kit.index(workspace, id, conversation).await;
    }
    (workspace, token, sam)
}

fn q(text: &str) -> String {
    let mut encoded = String::from("q=");
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(char::from(byte));
            }
            _ => {
                let _ = write!(encoded, "%{byte:02X}");
            }
        }
    }
    encoded
}

#[sqlx::test(migrations = "../../../migrations")]
async fn anonymous_and_outsiders_are_refused(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, _, _) = mailbox(&kit).await;
    let anonymous = kit.search("", workspace, &q("fatura")).await;
    assert_eq!(anonymous.status, 401);
    let (stranger, _) = kit.member("rui", WorkspaceId::new());
    let other = kit.search(&stranger, workspace, &q("fatura")).await;
    assert_eq!(other.status, 404);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn accents_stems_and_languages(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, sam, _) = mailbox(&kit).await;
    assert_eq!(
        kit.ids(&sam, workspace, &q("fatura joao")).await,
        ["c-water"]
    );
    assert_eq!(kit.ids(&sam, workspace, &q("FATURAS")).await, ["c-water"]);
    assert_eq!(kit.ids(&sam, workspace, &q("fuites")).await, ["c-fuite"]);
    assert_eq!(kit.ids(&sam, workspace, &q("reviewing")).await, ["c-q4"]);
    // A code and an attachment name.
    assert_eq!(
        kit.ids(&sam, workspace, &q("F-2026-0457")).await,
        ["c-factura"]
    );
    assert_eq!(kit.ids(&sam, workspace, &q("09812")).await, ["c-water"]);
    // Every word must match.
    assert!(
        kit.ids(&sam, workspace, &q("fatura boiler"))
            .await
            .is_empty()
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_query_can_be_limited_to_kinds_of_record(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, sam, _) = mailbox(&kit).await;
    let with = |types: &str| format!("{}&types={types}", q("fatura"));
    // The kind that is searched, alone or in a list.
    for types in [
        "conversations.conversation",
        "pages.page,conversations.conversation",
    ] {
        assert_eq!(
            kit.ids(&sam, workspace, &with(types)).await,
            ["c-water"],
            "{types}"
        );
    }
    // A kind that holds nothing here, one nobody has, and an empty list find
    // nothing: the others are not searched in their place.
    for types in ["pages.page", "nothing.at_all", ""] {
        assert!(
            kit.ids(&sam, workspace, &with(types)).await.is_empty(),
            "{types}"
        );
    }
    // No limit is everything the caller may search, as before.
    assert_eq!(kit.ids(&sam, workspace, &q("fatura")).await, ["c-water"]);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn people_partially(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, sam, _) = mailbox(&kit).await;
    assert_eq!(kit.ids(&sam, workspace, &q("from:lope")).await, ["c-q4"]);
    assert_eq!(
        kit.ids(&sam, workspace, &q("from:aguas")).await,
        ["c-water"]
    );
    assert_eq!(kit.ids(&sam, workspace, &q("lopes")).await, ["c-q4"]);
    let mut to_sam = kit.ids(&sam, workspace, &q("to:sam@example")).await;
    to_sam.sort();
    assert_eq!(to_sam.len(), 4);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn operators_combined(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, sam, _) = mailbox(&kit).await;
    kit.edit(workspace, "c-q4", |c| c.unread = false);
    kit.announce(workspace, "c-q4").await;
    kit.run_jobs().await;
    let response = kit
        .search(
            &sam,
            workspace,
            &q("has:attachment is:unread newer_than:30d fatura"),
        )
        .await;
    assert_eq!(response.status, 200);
    let hits = response.json["hits"].as_array().unwrap();
    assert_eq!(hits.len(), 1, "{}", response.json);
    assert_eq!(hits[0]["record"]["id"], "c-water");
    assert_eq!(hits[0]["facets"]["attachment"], true);
    assert_eq!(hits[0]["facets"]["unread"], true);
    assert_eq!(hits[0]["matched"]["id"], "m-water");
    assert_eq!(
        hits[0]["from"]["address"],
        "faturacao@aguas-do-vale.example"
    );
    let filters: Vec<String> = response.json["query"]["filters"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| {
            format!(
                "{}:{}",
                f["operator"].as_str().unwrap(),
                f["value"].as_str().unwrap()
            )
        })
        .collect();
    assert_eq!(filters, ["has:attachment", "is:unread", "newer_than:30d"]);
    // The read conversation has an attachment but is not unread.
    assert!(
        kit.ids(&sam, workspace, &q("is:unread plan"))
            .await
            .is_empty()
    );
    assert_eq!(kit.ids(&sam, workspace, &q("is:read")).await, ["c-q4"]);
    assert_eq!(
        kit.ids(&sam, workspace, &q("subject:septiembre")).await,
        ["c-factura"]
    );
    assert_eq!(
        kit.ids(&sam, workspace, &q("older_than:15d")).await,
        ["c-factura"]
    );
    assert_eq!(
        kit.ids(&sam, workspace, &q("after:2000-01-01 before:2000-12-31"))
            .await,
        Vec::<String>::new()
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn half_typed_operators_answer_with_a_warning(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, sam, _) = mailbox(&kit).await;
    let response = kit.search(&sam, workspace, &q("invoice from:")).await;
    assert_eq!(response.status, 200);
    assert_eq!(response.json["hits"][0]["record"]["id"], "c-q4");
    assert_eq!(response.json["query"]["warnings"][0]["code"], "empty_value");
    assert_eq!(response.json["query"]["warnings"][0]["operator"], "from");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn spam_and_trash_only_when_asked(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, sam, _) = mailbox(&kit).await;
    kit.edit(workspace, "c-fuite", |c| {
        c.messages[0] = c.messages[0].clone().in_mailbox("trash");
    });
    kit.announce(workspace, "c-fuite").await;
    kit.run_jobs().await;
    assert!(kit.ids(&sam, workspace, &q("fuite")).await.is_empty());
    assert_eq!(
        kit.ids(&sam, workspace, &q("fuite in:trash")).await,
        ["c-fuite"]
    );
    assert_eq!(
        kit.ids(&sam, workspace, &q("fuite in:anywhere")).await,
        ["c-fuite"]
    );
    assert!(
        kit.ids(&sam, workspace, &q("fuite in:inbox"))
            .await
            .is_empty()
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_last_word_as_a_prefix(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, sam, _) = mailbox(&kit).await;
    assert_eq!(
        kit.ids(&sam, workspace, &format!("{}&prefix=true", q("boiler rep")))
            .await,
        ["c-q4"]
    );
    assert!(kit.ids(&sam, workspace, &q("boiler rep")).await.is_empty());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn pages_and_cursors(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (sam, user) = kit.member("sam", workspace);
    kit.share(workspace, "inbox-sam", &[user]);
    for i in 0..7 {
        let id = format!("c-{i}");
        kit.put(
            workspace,
            &id,
            Conversation::new(
                "inbox-sam",
                &format!("Receipt {i}"),
                vec![Message::new(
                    &format!("m-{i}"),
                    "shop@example.com",
                    "Your receipt is attached",
                    i,
                )],
            ),
        );
        kit.announce(workspace, &id).await;
    }
    kit.run_jobs().await;
    for sort in ["newest", "relevance"] {
        let mut seen = Vec::new();
        let mut cursor: Option<String> = None;
        loop {
            let mut query = format!("{}&sort={sort}&limit=3", q("receipt"));
            if let Some(cursor) = &cursor {
                let _ = write!(query, "&cursor={cursor}");
            }
            let response = kit.search(&sam, workspace, &query).await;
            assert_eq!(response.status, 200, "{}", response.json);
            for hit in response.json["hits"].as_array().unwrap() {
                seen.push(hit["record"]["id"].as_str().unwrap().to_owned());
            }
            match response.json["next_cursor"].as_str() {
                Some(next) => cursor = Some(next.to_owned()),
                None => break,
            }
        }
        if sort == "newest" {
            assert_eq!(seen, ["c-0", "c-1", "c-2", "c-3", "c-4", "c-5", "c-6"]);
        }
        let mut unique = seen.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), 7, "{sort}: {seen:?}");
    }
    let first = kit
        .search(&sam, workspace, &format!("{}&limit=3", q("receipt")))
        .await;
    let cursor = first.json["next_cursor"].as_str().unwrap();
    let mismatch = kit
        .search(&sam, workspace, &format!("{}&cursor={cursor}", q("shop")))
        .await;
    assert_eq!(mismatch.status, 400);
    assert_eq!(mismatch.json["code"], "cursor_mismatch");
    let too_long = kit.search(&sam, workspace, &q(&"a".repeat(513))).await;
    assert_eq!(too_long.status, 400);
    assert_eq!(too_long.json["code"], "query_too_long");
    let bad_limit = kit
        .search(&sam, workspace, &format!("{}&limit=51", q("x")))
        .await;
    assert_eq!(bad_limit.status, 400);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn highlights_are_text_and_mail_cannot_forge_them(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (sam, user) = kit.member("sam", workspace);
    kit.share(workspace, "inbox-sam", &[user]);
    kit.index(
        workspace,
        "c-x",
        Conversation::new(
            "inbox-sam",
            "Urgent invoice",
            vec![Message::new(
                "m-x",
                "ops@example.com",
                "Please pay the <b>urgent</b> \u{e000}fake\u{e001} invoice today.",
                1,
            )],
        ),
    )
    .await;
    let response = kit.search(&sam, workspace, &q("invoice")).await;
    let hit = &response.json["hits"][0];
    let text: String = hit["snippet"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["text"].as_str().unwrap())
        .collect();
    // Tag-like text is dropped by the text search parser, never returned
    // as markup; the words stay.
    assert!(text.contains("urgent") && !text.contains('<'), "{text}");
    assert!(!text.contains('\u{e000}') && !text.contains('\u{e001}'));
    let hits: Vec<&str> = hit["snippet"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|s| s["hit"] == true)
        .map(|s| s["text"].as_str().unwrap())
        .collect();
    assert_eq!(hits, ["invoice"]);
    let title_hits: Vec<&str> = hit["title"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|s| s["hit"] == true)
        .map(|s| s["text"].as_str().unwrap())
        .collect();
    assert_eq!(title_hits, ["invoice"]);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn only_inboxes_the_caller_may_see(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, sam, sam_id) = mailbox(&kit).await;
    let (rui, rui_id) = kit.member("rui", workspace);
    kit.share(workspace, "inbox-rui", &[rui_id]);
    kit.index(
        workspace,
        "c-rui",
        Conversation::new(
            "inbox-rui",
            "Boiler repair quote",
            vec![Message::new(
                "m-rui",
                "plumber@example.com",
                "The boiler repair costs 90 €.",
                1,
            )],
        ),
    )
    .await;
    assert_eq!(kit.ids(&rui, workspace, &q("boiler")).await, ["c-rui"]);
    assert_eq!(kit.ids(&sam, workspace, &q("boiler")).await, ["c-q4"]);
    // Removed from the inbox: gone from results at once.
    kit.share(workspace, "inbox-sam", &[]);
    assert!(kit.ids(&sam, workspace, &q("boiler")).await.is_empty());
    kit.share(workspace, "inbox-sam", &[sam_id]);
    // A client token bound to the workspace sees every inbox, but only with
    // a scope that reads conversations.
    let client = kit.client(workspace, "search:read conversations:read");
    let mut all = kit.ids(&client, workspace, &q("boiler")).await;
    all.sort();
    assert_eq!(all, ["c-q4", "c-rui"]);
    let blind = kit.client(workspace, "search:read");
    assert!(kit.ids(&blind, workspace, &q("boiler")).await.is_empty());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn current_state_wins_whatever_the_order(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, sam, _) = mailbox(&kit).await;
    // Starred then unstarred: two deliveries, both read the final state.
    kit.edit(workspace, "c-fuite", |c| c.starred = true);
    kit.announce(workspace, "c-fuite").await;
    kit.edit(workspace, "c-fuite", |c| c.starred = false);
    kit.announce(workspace, "c-fuite").await;
    kit.run_jobs().await;
    assert!(kit.ids(&sam, workspace, &q("is:starred")).await.is_empty());
    // A message removed: its words go, the conversation's others stay.
    kit.edit(workspace, "c-q4", |c| {
        c.messages.remove(1);
    });
    kit.announce(workspace, "c-q4").await;
    kit.run_jobs().await;
    assert!(kit.ids(&sam, workspace, &q("thursday")).await.is_empty());
    assert_eq!(kit.ids(&sam, workspace, &q("budget")).await, ["c-q4"]);
    // The conversation removed: gone.
    kit.remove(workspace, "c-q4");
    kit.announce(workspace, "c-q4").await;
    kit.run_jobs().await;
    assert!(kit.ids(&sam, workspace, &q("budget")).await.is_empty());
    let left: i64 =
        sqlx::query_scalar("SELECT count(*) FROM search.entries WHERE record_id LIKE 'm-q4-%'")
            .fetch_one(&kit.db)
            .await
            .unwrap();
    assert_eq!(left, 0);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_flag_change_reads_no_text(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, sam, _) = mailbox(&kit).await;
    let before = kit.source.texts_read.load(Ordering::SeqCst);
    kit.edit(workspace, "c-q4", |c| c.starred = true);
    kit.announce(workspace, "c-q4").await;
    kit.run_jobs().await;
    assert_eq!(kit.source.texts_read.load(Ordering::SeqCst), before);
    assert_eq!(kit.ids(&sam, workspace, &q("is:starred")).await, ["c-q4"]);
    // A new title re-indexes the entries' vectors from stored text.
    kit.edit(workspace, "c-q4", |c| {
        "Quarterly roadmap".clone_into(&mut c.subject);
    });
    kit.announce(workspace, "c-q4").await;
    kit.run_jobs().await;
    assert_eq!(kit.source.texts_read.load(Ordering::SeqCst), before);
    assert_eq!(kit.ids(&sam, workspace, &q("roadmap")).await, ["c-q4"]);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_workspace_that_predates_search_is_backfilled(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (sam, user) = kit.member("sam", workspace);
    kit.share(workspace, "inbox-sam", &[user]);
    for i in 0..150 {
        kit.put(
            workspace,
            &format!("c-{i:03}"),
            Conversation::new(
                "inbox-sam",
                &format!("Old newsletter {i}"),
                vec![Message::new(
                    &format!("m-{i:03}"),
                    "news@example.com",
                    "Archive issue",
                    400,
                )],
            ),
        );
    }
    let first = kit.search(&sam, workspace, &q("newsletter")).await;
    assert_eq!(first.status, 200);
    assert_eq!(first.json["indexing"], true);
    assert!(first.json["hits"].as_array().unwrap().is_empty());
    kit.run_jobs().await;
    let after = kit
        .search(&sam, workspace, &format!("{}&limit=50", q("newsletter")))
        .await;
    assert_eq!(after.json["indexing"], false);
    assert_eq!(after.json["hits"].as_array().unwrap().len(), 50);
    let indexed: i64 = sqlx::query_scalar("SELECT count(*) FROM search.groups")
        .fetch_one(&kit.db)
        .await
        .unwrap();
    assert_eq!(indexed, 150);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_deleted_workspace_is_purged_and_stays_unindexed(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, _, _) = mailbox(&kit).await;
    let mut tx = kit.state.db.begin().await.unwrap();
    campbooks_kernel::db::set_workspace(&mut tx, &workspace.to_string())
        .await
        .unwrap();
    kit.state
        .events
        .publish(
            &mut tx,
            &crate::events::WorkspaceDeleting {
                workspace: workspace.to_string(),
                purge_after: chrono::Utc::now(),
            },
        )
        .await
        .unwrap();
    tx.commit().await.unwrap();
    kit.run_jobs().await;
    let count = |table: &'static str| {
        let db = kit.db.clone();
        async move {
            sqlx::query_scalar::<_, i64>(sqlx::AssertSqlSafe(format!(
                "SELECT count(*) FROM {table} WHERE workspace_id = $1"
            )))
            .bind(workspace.as_uuid())
            .fetch_one(&db)
            .await
            .unwrap()
        }
    };
    assert_eq!(count("search.groups").await, 0);
    assert_eq!(count("search.entries").await, 0);
    // A late delivery indexes nothing and starts no backfill.
    kit.announce(workspace, "c-q4").await;
    kit.run_jobs().await;
    assert_eq!(count("search.groups").await, 0);
    let states: Vec<String> =
        sqlx::query_scalar("SELECT state FROM search.backfills WHERE workspace_id = $1")
            .bind(workspace.as_uuid())
            .fetch_all(&kit.db)
            .await
            .unwrap();
    assert_eq!(states, ["deleting"]);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_search_does_not_wait_for_a_backfill_page(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (sam, user) = kit.member("sam", workspace);
    kit.share(workspace, "inbox-sam", &[user]);
    kit.put(
        workspace,
        "c-old",
        Conversation::new(
            "inbox-sam",
            "Old receipt",
            vec![Message::new("m-old", "shop@example.com", "Receipt", 30)],
        ),
    );
    // The first search starts the backfill; its page then stops half-way,
    // its transaction open.
    let gate = std::sync::Arc::new(tokio::sync::Notify::new());
    *kit.source.page_gate.lock().unwrap() = Some(std::sync::Arc::clone(&gate));
    assert_eq!(kit.search(&sam, workspace, &q("receipt")).await.status, 200);
    let worker = {
        let state = kit.state.clone();
        tokio::spawn(async move {
            campbooks_kernel::jobs::Worker::new(state)
                .run_until_idle()
                .await
        })
    };
    kit.source.page_entered.notified().await;
    let during = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        kit.search(&sam, workspace, &q("receipt")),
    )
    .await
    .expect("a search while a backfill page runs answers at once");
    assert_eq!(during.status, 200);
    assert_eq!(during.json["indexing"], true);
    gate.notify_one();
    worker.await.unwrap();
    let after = kit.search(&sam, workspace, &q("receipt")).await;
    assert_eq!(after.json["indexing"], false);
    assert_eq!(after.json["hits"].as_array().unwrap().len(), 1);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn cursors_belong_to_their_caller_and_cannot_be_altered(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, sam, sam_id) = mailbox(&kit).await;
    let (rui, rui_id) = kit.member("rui", workspace);
    kit.share(workspace, "inbox-sam", &[sam_id, rui_id]);
    let page = format!("{}&limit=1&sort=newest", q("to:sam"));
    let first = kit.search(&sam, workspace, &page).await;
    let cursor = first.json["next_cursor"].as_str().unwrap().to_owned();
    let mine = kit
        .search(&sam, workspace, &format!("{page}&cursor={cursor}"))
        .await;
    assert_eq!(mine.status, 200);
    let theirs = kit
        .search(&rui, workspace, &format!("{page}&cursor={cursor}"))
        .await;
    assert_eq!(theirs.status, 400);
    assert_eq!(theirs.json["code"], "cursor_mismatch");
    // An altered payload (another position) no longer verifies.
    let (payload, tag) = cursor.split_once('.').unwrap();
    let mut forged = payload.to_owned();
    let last = forged.pop().unwrap();
    forged.push(if last == 'A' { 'B' } else { 'A' });
    let altered = kit
        .search(&sam, workspace, &format!("{page}&cursor={forged}.{tag}"))
        .await;
    assert_eq!(altered.status, 400);
    let unsigned = kit
        .search(&sam, workspace, &format!("{page}&cursor={payload}"))
        .await;
    assert_eq!(unsigned.status, 400);
    // The same caller's cursor never pages another workspace.
    let elsewhere = WorkspaceId::new();
    kit.member("sam", elsewhere);
    let other = kit
        .search(&sam, elsewhere, &format!("{page}&cursor={cursor}"))
        .await;
    assert_eq!(other.status, 400);
    assert_eq!(other.json["code"], "cursor_mismatch");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn exclusions_alone_and_impossible_dates(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, sam, _) = mailbox(&kit).await;
    let excluded = kit.search(&sam, workspace, &q("-fatura -plan")).await;
    assert_eq!(excluded.status, 400);
    assert_eq!(excluded.json["code"], "query_only_excludes");
    // Filters with an exclusion are fine.
    assert_eq!(
        kit.search(&sam, workspace, &q("is:unread -fatura"))
            .await
            .status,
        200
    );
    for query in [
        "after:99999-01-01 fatura",
        "before:0000-12-31 fatura",
        "older_than:5é",
    ] {
        let response = kit.search(&sam, workspace, &q(query)).await;
        assert_eq!(response.status, 200, "{query}: {}", response.json);
        assert_eq!(
            response.json["query"]["warnings"][0]["code"],
            "invalid_value"
        );
    }
}

/// The instant query on a big workspace: 20,000 conversations, 100,000
/// generated fictional messages. Run it by hand on the reference host:
/// `cargo test -p campbooks-search --lib bench_instant_queries -- --ignored --nocapture`.
#[sqlx::test(migrations = "../../../migrations")]
#[ignore = "benchmark; run by hand (see the doc comment)"]
async fn bench_instant_queries(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (sam, user) = kit.member("sam", workspace);
    kit.share(workspace, "inbox-sam", &[user]);
    sqlx::query(
        "INSERT INTO search.groups (id, workspace_id, record_type, record_id, container_type,
                                    container_id, occurred_at, title, title_folded, facets,
                                    index_version)
         SELECT gen_random_uuid(), $1, 'conversations.conversation', 'g' || i,
                'conversations.inbox', 'inbox-sam', now() - make_interval(mins => i),
                'Subject ' || i, 'subject ' || i, '{\"unread\": false}', 1
           FROM generate_series(1, 20000) i",
    )
    .bind(workspace.as_uuid())
    .execute(&kit.db)
    .await
    .unwrap();
    sqlx::query(
        "WITH words AS (SELECT ARRAY['invoice', 'meeting', 'boiler', 'repair', 'budget',
                                     'fatura', 'factura', 'reçu', 'plan', 'friday', 'water',
                                     'contract', 'delivery', 'order', 'payment', 'receipt',
                                     'review', 'board', 'notes', 'thursday'] AS w)
         INSERT INTO search.entries (id, workspace_id, group_id, record_type, record_id,
                                     occurred_at, language, people_words, from_folded, body,
                                     facets, stamp, tsv)
         SELECT gen_random_uuid(), $1, g.id, 'conversations.message', g.record_id || '-' || k,
                g.occurred_at, 'en', 'ana lopes example com', 'ana lopes ana@example.com',
                b.body, '{\"mailbox\": \"inbox\"}', 's',
                search.entry_vector(g.title, 'ana lopes example com', '', b.body, 'en')
           FROM search.groups g CROSS JOIN generate_series(1, 5) k
           CROSS JOIN LATERAL (
               SELECT string_agg(words.w[1 + floor(random() * 20)::int], ' ') AS body
                 FROM words, generate_series(1, 80 + k - k)
           ) b
          WHERE g.workspace_id = $1",
    )
    .bind(workspace.as_uuid())
    .execute(&kit.db)
    .await
    .unwrap();
    sqlx::query("ANALYZE").execute(&kit.db).await.unwrap();
    for query in [
        "q=boi&prefix=true",
        "q=boiler%20rep&prefix=true",
        "q=invoice%20friday",
        "q=from%3Alope%20budget",
        "q=receipt&sort=newest",
    ] {
        let mut times = Vec::new();
        for _ in 0..20 {
            let started = std::time::Instant::now();
            let response = kit.search(&sam, workspace, query).await;
            times.push(started.elapsed());
            assert_eq!(response.status, 200);
        }
        times.sort();
        eprintln!("{query}: p50 {:?}, p95 {:?}", times[10], times[18]);
    }
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_removed_inbox_leaves_the_index(db: PgPool) {
    use campbooks_kernel::db::BeginFor as _;

    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (sam, user) = kit.member("sam", workspace);
    kit.share(workspace, "inbox-sam", &[user]);
    for (id, inbox) in [("c-gone", "inbox-sam"), ("c-kept", "inbox-other")] {
        kit.index(
            workspace,
            id,
            Conversation::new(
                inbox,
                "Quarterly invoice",
                vec![Message::new(
                    &format!("m-{id}"),
                    "ops@example.com",
                    "Invoice",
                    1,
                )],
            ),
        )
        .await;
    }
    assert_eq!(
        kit.search(&sam, workspace, &q("invoice")).await.json["hits"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let mut tx = kit.state.db.begin_for(workspace).await.unwrap();
    kit.state
        .events
        .publish(
            &mut tx,
            &crate::events::InboxRemoved {
                workspace: workspace.to_string(),
                inbox: "inbox-sam".to_owned(),
            },
        )
        .await
        .unwrap();
    tx.commit().await.unwrap();
    kit.run_jobs().await;
    let left: Vec<String> = sqlx::query_scalar("SELECT container_id FROM search.groups")
        .fetch_all(&kit.db)
        .await
        .unwrap();
    assert_eq!(left, ["inbox-other"]);
    let found = kit.search(&sam, workspace, &q("invoice")).await;
    assert_eq!(found.json["hits"], serde_json::json!([]));
}
