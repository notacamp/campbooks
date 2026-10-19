//! `GET /api/v1/w/{workspace_id}/conversations/{conversation_id}/messages/{message_id}/body`
//! — one message's text and HTML bodies, as JSON.
//!
//! The HTML is untrusted and never served as `text/html`: it is sanitised
//! here (`domain::sanitize`: style sheets inlined into filtered `style`
//! attributes; no scripts, forms or frames; links open in a new tab; remote
//! images held back unless asked for or a smart rule trusts the sender;
//! tracking pixels removed; a reply's trailing quoted history wrapped in
//! `<div data-cb-quoted="1">` for the reader to fold), then reaches
//! the client as a string that the client renders only in its sandboxed
//! frame (`lib/ui/safe-html`, AGENTS.md), with the message's filtered
//! `@media` rules beside it.
//! Inline (`cid:`) images are not remote: they show the message's own parts
//! as `data:` URLs, raster types only and within [`MAX_INLINE_IMAGE_BYTES`]
//! and [`MAX_INLINE_TOTAL_BYTES`]; anything else shows the placeholder.

use axum::{Json, extract::State};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, Result,
    blobs::BlobRef,
    db::BeginFor as _,
    extract,
    tenancy::{Workspace, WorkspaceId},
};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::domain::resources;
use crate::events::MESSAGE_RECORD;
use crate::{
    domain::paths::MessagePath,
    domain::{
        access::visible_inboxes,
        attachments::{is_raster, sniff},
        blobs::{Budget, read_bytes, read_text, words_until},
        plain_text::Quotes,
        rules::{ImagesHeld, images},
        sanitize::{InlineImages, sanitize},
    },
    features::get_conversation::Bodies,
    model::{ConversationId, InboxId, MessageId},
    scopes::ConversationsRead,
};
use campbooks_kernel::RecordRef;
use campbooks_kernel::authz::Action;

/// Longest body returned, in bytes, per part.
pub const MAX_BODY_BYTES: usize = 2 * 1024 * 1024;
/// Largest inline image shown, in bytes; a larger one shows the placeholder.
pub const MAX_INLINE_IMAGE_BYTES: u64 = 2 * 1024 * 1024;
/// Most inline-image bytes shown in one body; the rest show the placeholder.
pub const MAX_INLINE_TOTAL_BYTES: u64 = 6 * 1024 * 1024;

/// How to render the HTML part.
#[derive(Debug, Clone, Copy, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct BodyQuery {
    /// `true` to keep remote images (the member chose "Show images");
    /// tracking pixels are removed either way.
    #[serde(default)]
    pub show_images: bool,
    /// `true` for the message's words only, as `text`: its text part when it
    /// has any, else its HTML read as text (at most half a second). `html`
    /// is then absent. What a reply or forward quotes when
    /// `conversations.get` marked the message `text_unread`.
    #[serde(default)]
    pub words: bool,
}

/// Bytes read of each part for [`BodyQuery::words`].
const WORDS_BUDGET: Budget = Budget {
    text: 256 * 1024,
    html: 256 * 1024,
};

/// A message's bodies.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "the wire shape: independent facts about one message's images"
)]
pub struct MessageBody {
    /// The plain-text part.
    pub text: Option<String>,
    /// The HTML part, sanitised; still to be shown only in a sandboxed frame.
    pub html: Option<String>,
    /// Whether either part was cut short.
    pub truncated: bool,
    /// Remote images in the HTML part: held back (their `src` removed)
    /// unless `show_images` was asked for.
    pub remote_images: u32,
    /// Whether the remote images were kept.
    pub images_shown: bool,
    /// Whether a smart rule trusts the sender with its images ("Always show
    /// images from …"), so they were kept without asking: the sender is
    /// authenticated and nothing about the mail looks suspicious.
    pub images_trusted: bool,
    /// Whether "Always show images from …" can take effect here: remote
    /// images were held back, no rule trusts the sender yet, and nothing
    /// about the email would keep its images back.
    pub images_offer: bool,
    /// The sender's domain the member may trust instead of the address
    /// (owned, not a free mail provider's), when it may be offered.
    pub images_domain: Option<String>,
    /// Why the remote images stay held back although the sender is, or
    /// could be, trusted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub images_held: Option<ImagesHeld>,
    /// The HTML part's `@media` rules, filtered like its `style` attributes
    /// (nothing that loads or runs survives): CSS the client applies to the
    /// frame's document only, never to its own page.
    pub css: Option<String>,
}

/// Read one message's text and HTML bodies.
#[utoipa::path(
    get,
    path = "/api/v1/w/{workspace_id}/conversations/{conversation_id}/messages/{message_id}/body",
    operation_id = "conversations.get_message_body",
    tag = "conversations",
    security(("bearer" = ["conversations:read"])),
    extensions(("x-campbooks" = json!(Operation::query().records(["conversations.message"]).access("conversations.message", campbooks_kernel::authz::Action::Read)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("conversation_id" = ConversationId, Path, description = "The conversation."),
        ("message_id" = MessageId, Path, description = "The message."),
        BodyQuery,
    ),
    responses(
        (status = OK, description = "The bodies.", body = MessageBody),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope.", body = Problem,
            content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such message, or you cannot see it.",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    workspace: Workspace<ConversationsRead>,
    extract::Path(path): extract::Path<MessagePath>,
    extract::Query(query): extract::Query<BodyQuery>,
) -> Result<Json<MessageBody>> {
    let (conversation, message) = (path.conversation_id, path.message_id);
    let access = &workspace.access;
    let mut tx = state.db.begin_for(access.workspace).await?;
    resources::authorize(
        &state,
        &mut tx,
        access,
        Action::Read,
        &RecordRef::new(MESSAGE_RECORD, &message),
    )
    .await?;
    let inboxes = visible_inboxes(&mut tx, access).await?;
    let bodies = sqlx::query_scalar!(
        r#"SELECT bodies FROM conversations.messages
            WHERE id = $1 AND conversation_id = $2 AND inbox_id = ANY($3)"#,
        message as MessageId,
        conversation as ConversationId,
        &inboxes as &[InboxId],
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(Error::NotFound)?;
    if query.words {
        tx.commit().await?;
        return words_of(&state, &access.workspace.to_string(), bodies).await;
    }
    // Parts `cid:` images may name (read only if the HTML names any).
    let parts: Vec<InlinePart> = sqlx::query_as!(
        InlinePart,
        r#"SELECT content_id AS "content_id!", content_type, blob
             FROM conversations.attachments
            WHERE message_id = $1 AND content_id IS NOT NULL
            ORDER BY position"#,
        message as MessageId,
    )
    .fetch_all(&mut *tx)
    .await?;
    let assessed = images(&mut tx, &state.binding, &inboxes, message).await?;
    let trusted = assessed.trusted;
    tx.commit().await?;
    let bodies: Bodies = serde_json::from_value(bodies)
        .map_err(|e| Error::internal(crate::domain::redact::json(&e)))?;
    let ws = access.workspace.to_string();
    let (text, text_cut) = read(&state, &ws, bodies.text.as_ref()).await?;
    let (html, html_cut) = read(&state, &ws, bodies.html.as_ref()).await?;
    let truncated = text_cut || html_cut;
    let inline = match html.as_deref() {
        Some(html) if html.to_ascii_lowercase().contains("cid:") => {
            inline_images(&state, &ws, parts).await?
        }
        _ => InlineImages::new(),
    };
    // CPU-bound (inlining, two ammonia passes): off the async workers.
    // Trust only flips what is held back by default: the same sanitiser,
    // tracking pixels removed either way. Mail in spam keeps them held back
    // even when asked for.
    let show = !assessed.spam && (query.show_images || trusted);
    let sanitized = match html {
        Some(html) => Some(
            tokio::task::spawn_blocking(move || sanitize(&html, show, &inline))
                .await
                .map_err(Error::internal)?,
        ),
        None => None,
    };
    let remote_images = sanitized.as_ref().map_or(0, |s| s.remote_images);
    // Nothing to offer or explain about a message with no remote images.
    let remote = remote_images > 0;
    Ok(Json(MessageBody {
        text,
        remote_images,
        images_shown: show,
        images_trusted: trusted,
        images_offer: remote && !show && assessed.offer,
        images_domain: assessed.domain.filter(|_| remote && !show),
        images_held: assessed.held.filter(|_| remote && !show),
        css: sanitized.as_ref().and_then(|s| s.css.clone()),
        html: sanitized.map(|s| s.html),
        truncated,
    }))
}

/// The message's words only ([`BodyQuery::words`]), read off the async
/// workers within the per-message reading time.
async fn words_of(
    state: &AppState,
    ws: &str,
    bodies: serde_json::Value,
) -> Result<Json<MessageBody>> {
    let bodies: Bodies = serde_json::from_value(bodies)
        .map_err(|e| Error::internal(crate::domain::redact::json(&e)))?;
    // Cut at the text budget, or only some of the HTML's words: either way
    // a quote of them says it is not whole.
    let text = if bodies.text.is_some() || bodies.html.is_some() {
        let read = words_until(
            &state.blobs,
            ws,
            (bodies.text.as_ref(), bodies.html.as_ref()),
            WORDS_BUDGET,
            Quotes::Keep,
            None,
        )
        .await?;
        Some((read.text, read.cut || read.partial))
    } else {
        None
    };
    Ok(Json(MessageBody {
        truncated: text.as_ref().is_some_and(|(_, cut)| *cut),
        text: text.map(|(words, _)| words),
        html: None,
        remote_images: 0,
        images_shown: false,
        images_trusted: false,
        images_offer: false,
        images_domain: None,
        images_held: None,
        css: None,
    }))
}

/// A part of the message with a content id.
struct InlinePart {
    content_id: String,
    content_type: String,
    blob: serde_json::Value,
}

/// `parts` of a raster type, as `data:` URLs by content id, within the size
/// limits.
async fn inline_images(state: &AppState, ws: &str, parts: Vec<InlinePart>) -> Result<InlineImages> {
    let mut images = InlineImages::new();
    let mut total = 0_u64;
    for part in parts {
        if !is_raster(&part.content_type) {
            continue;
        }
        let blob: BlobRef = serde_json::from_value(part.blob)
            .map_err(|e| Error::internal(crate::domain::redact::json(&e)))?;
        if blob.size > MAX_INLINE_IMAGE_BYTES || total + blob.size > MAX_INLINE_TOTAL_BYTES {
            continue;
        }
        let limit = usize::try_from(MAX_INLINE_IMAGE_BYTES).unwrap_or(usize::MAX);
        let (bytes, cut) = read_bytes(&state.blobs, ws, &blob, limit).await?;
        if cut {
            continue;
        }
        // Shown only as what its bytes are, and only if that is a picture.
        let Some(kind) = sniff(&bytes).filter(|kind| is_raster(kind)) else {
            continue;
        };
        total += blob.size;
        let id = part
            .content_id
            .trim()
            .trim_start_matches('<')
            .trim_end_matches('>')
            .to_lowercase();
        let url = format!("data:{};base64,{}", kind, STANDARD.encode(bytes));
        images.entry(id).or_insert(url);
    }
    Ok(images)
}

async fn read(
    state: &AppState,
    ws: &str,
    blob: Option<&BlobRef>,
) -> Result<(Option<String>, bool)> {
    match blob {
        Some(blob) => {
            let (text, cut) = read_text(&state.blobs, ws, blob, MAX_BODY_BYTES).await?;
            Ok((Some(text), cut))
        }
        None => Ok((None, false)),
    }
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use serde_json::json;
    use sqlx::PgPool;

    use crate::{dev, domain::sanitize::PLACEHOLDER, testkit::Kit};

    #[sqlx::test(migrations = "../../../migrations")]
    async fn inline_images_show_their_parts_and_the_rest_a_placeholder(db: PgPool) {
        let kit = Kit::new(db.clone()).await;
        let (workspace, inbox, sam) = kit.sams_inbox().await;
        let recorded = json!({"changes": [{"op": "upsert", "message": {
            "provider_message_id": "cid-1",
            "from": {"name": "Ana Lopes", "address": "ana@brightloop.example"},
            "to": [{"address": "sam@example.com"}],
            "subject": "Our new logo",
            "sent_at": "2026-10-06T08:30:00Z",
            "direction": "inbound",
            "text": "Our new logo.",
            "html": "<p><img src=\"cid:logo@brightloop.example\" alt=\"Logo\"><img src=\"cid:badge@brightloop.example\" alt=\"Badge\"></p><p><img src=\"cid:vector@brightloop.example\" alt=\"Vector\"></p><img src=\"https://cdn.brightloop.example/banner.png\" alt=\"Banner\" width=\"560\" height=\"120\">",
            "attachments": [
                {"filename": "logo.gif", "content_type": "image/gif", "content": "GIF89a-bytes",
                 "inline": true, "content_id": "logo@brightloop.example"},
                {"filename": "badge.png", "content_type": "image/png",
                 "content": "<html><script>alert(1)</script></html>",
                 "inline": true, "content_id": "badge@brightloop.example"},
                {"filename": "vector.svg", "content_type": "image/svg+xml",
                 "content": "<svg xmlns='http://www.w3.org/2000/svg'><script>alert(1)</script></svg>",
                 "inline": true, "content_id": "vector@brightloop.example"}
            ],
            "mailbox": {"in_inbox": true},
            "read": false, "starred": false
        }}]});
        let batch = dev::record(&kit.state, workspace, inbox, &recorded)
            .await
            .unwrap();
        dev::submit(&kit.state, workspace, &batch).await.unwrap();
        kit.run_jobs().await;
        let (conversation, message): (String, String) = sqlx::query_as(
            "SELECT conversation_id::text, id::text FROM conversations.messages
              WHERE provider_message_id = 'cid-1'",
        )
        .fetch_one(&db)
        .await
        .unwrap();
        let stored: Option<String> = sqlx::query_scalar(
            "SELECT content_id FROM conversations.attachments WHERE filename = 'logo.gif'",
        )
        .fetch_one(&db)
        .await
        .unwrap();
        assert_eq!(stored.as_deref(), Some("logo@brightloop.example"));

        let uri =
            format!("/api/v1/w/{workspace}/conversations/{conversation}/messages/{message}/body");
        let body = kit.get(&sam, &uri).await;
        assert_eq!(body.status, StatusCode::OK);
        let html = body.json["html"].as_str().unwrap();
        // The GIF part, without asking; never counted as remote.
        assert!(
            html.contains("src=\"data:image/gif;base64,R0lGODlhLWJ5dGVz\""),
            "{html}"
        );
        assert_eq!(body.json["remote_images"], 1);
        // The SVG part is not a raster image, the "PNG" is markup, and the
        // banner is held back: all show the placeholder; nothing of them
        // leaves.
        assert_eq!(html.matches(PLACEHOLDER).count(), 3, "{html}");
        assert!(!html.contains("script"), "{html}");
        assert!(!html.contains("cdn.brightloop.example"), "{html}");
        assert!(!html.contains("cid:"), "{html}");
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn style_sheets_are_inlined_and_media_rules_come_back_filtered(db: PgPool) {
        let kit = Kit::new(db.clone()).await;
        let (workspace, inbox, sam) = kit.sams_inbox().await;
        let recorded = json!({"changes": [{"op": "upsert", "message": {
            "provider_message_id": "styled-1",
            "from": {"name": "The Weekly Ledger", "address": "hello@weekly-ledger.example"},
            "to": [{"address": "sam@example.com"}],
            "subject": "Styled",
            "sent_at": "2026-10-06T08:30:00Z",
            "direction": "inbound",
            "text": "Styled.",
            "html": "<html><head><title>Ledger</title><link rel=stylesheet href=\"https://t.example/a.css\"><style>@import url(https://t.example/b.css); .lead { color: #3d4451; background: url(https://t.example/c.gif) } .pre { display: none } @media (max-width: 600px) { .c { width: 100% !important; background-image: url(https://t.example/d.gif) } } @media (prefers-color-scheme: dark) { .lead { color: #fff } }</style></head><body bgcolor=\"#eeeeee\"><span class=\"pre\">Preview</span><div class=\"c\" style=\"width: 600px\"><p class=\"lead\">Hi</p></div></body></html>",
            "mailbox": {"in_inbox": true},
            "read": false, "starred": false
        }}]});
        let batch = dev::record(&kit.state, workspace, inbox, &recorded)
            .await
            .unwrap();
        dev::submit(&kit.state, workspace, &batch).await.unwrap();
        kit.run_jobs().await;
        let (conversation, message): (String, String) = sqlx::query_as(
            "SELECT conversation_id::text, id::text FROM conversations.messages
              WHERE provider_message_id = 'styled-1'",
        )
        .fetch_one(&db)
        .await
        .unwrap();
        let uri =
            format!("/api/v1/w/{workspace}/conversations/{conversation}/messages/{message}/body");
        let body = kit.get(&sam, &uri).await;
        assert_eq!(body.status, StatusCode::OK);
        let html = body.json["html"].as_str().unwrap();
        assert_eq!(
            html,
            r##"<div bgcolor="#eeeeee"><span class="pre" style="display: none">Preview</span><div class="c" style="width: 600px"><p class="lead" style="color: #3d4451">Hi</p></div></div>"##
        );
        assert_eq!(
            body.json["css"],
            "@media (max-width: 600px) {\n.c { width: 100% !important }\n}"
        );
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn html_is_returned_as_json_data_never_as_a_page(db: PgPool) {
        let kit = Kit::new(db.clone()).await;
        let (workspace, _, sam) = kit.sams_inbox().await;
        let (conversation, message): (String, String) = sqlx::query_as(
            "SELECT conversation_id::text, id::text FROM conversations.messages
              WHERE provider_message_id = 'en-3'",
        )
        .fetch_one(&db)
        .await
        .unwrap();
        let uri =
            format!("/api/v1/w/{workspace}/conversations/{conversation}/messages/{message}/body");
        let response = kit.get(&sam, &uri).await;
        assert_eq!(response.status, StatusCode::OK);
        assert_eq!(response.content_type.as_deref(), Some("application/json"));
        assert!(
            response.json["html"]
                .as_str()
                .unwrap()
                .starts_with("<h1>Issue 142")
        );
        assert!(
            response.json["text"]
                .as_str()
                .unwrap()
                .contains("calmer mornings")
        );
        assert_eq!(response.json["truncated"], false);
        // Sanitised: the remote image is held back (the placeholder shows
        // in its place) and counted.
        assert!(
            !response.json["html"]
                .as_str()
                .unwrap()
                .contains("paperkite.example")
        );
        assert!(
            response.json["html"]
                .as_str()
                .unwrap()
                .contains(PLACEHOLDER)
        );
        assert_eq!(response.json["remote_images"], 1);
        assert_eq!(response.json["images_shown"], false);
        let shown = kit.get(&sam, &format!("{uri}?show_images=true")).await;
        assert_eq!(shown.status, StatusCode::OK);
        assert!(
            shown.json["html"]
                .as_str()
                .unwrap()
                .contains(r#"src="https://paperkite.example/pixel.gif""#)
        );
        assert_eq!(shown.json["images_shown"], true);

        let other = kit.conversation_of("en-1").await;
        let mismatched =
            format!("/api/v1/w/{workspace}/conversations/{other}/messages/{message}/body");
        assert_eq!(
            kit.get(&sam, &mismatched).await.status,
            StatusCode::NOT_FOUND
        );
    }

    /// One recorded message from `from` with a remote image and a tracking
    /// pixel, and the receiving provider's `results` (or none).
    fn newsletter(id: &str, subject: &str, results: Option<&str>) -> serde_json::Value {
        let mut message = json!({
            "provider_message_id": id,
            "from": {"name": "Brightloop", "address": "news@brightloop.example"},
            "to": [{"address": "sam@example.com"}],
            "subject": subject,
            "sent_at": "2026-10-06T08:30:00Z",
            "direction": "inbound",
            "text": "This month at Brightloop.",
            "html": "<p>This month</p><img src=\"https://cdn.brightloop.example/banner.png\" alt=\"Banner\" width=\"560\" height=\"120\"><img src=\"https://t.brightloop.example/open.gif\" width=\"1\" height=\"1\">",
            "mailbox": {"in_inbox": true},
            "read": false, "starred": false
        });
        if let Some(results) = results {
            message["headers"] = json!({"authentication_results": results});
        }
        json!({"op": "upsert", "message": message})
    }

    /// The body of message `id`, as Sam reads it.
    async fn body_of(
        kit: &Kit,
        sam: &str,
        workspace: campbooks_kernel::tenancy::WorkspaceId,
        id: &str,
    ) -> serde_json::Value {
        let (conversation, message): (String, String) = sqlx::query_as(
            "SELECT conversation_id::text, id::text FROM conversations.messages
              WHERE provider_message_id = $1",
        )
        .bind(id)
        .fetch_one(&kit.db)
        .await
        .unwrap();
        let uri =
            format!("/api/v1/w/{workspace}/conversations/{conversation}/messages/{message}/body");
        kit.get(sam, &uri).await.json
    }

    /// Every message stored as judged not suspicious by the model.
    async fn judged_not_suspicious(db: &PgPool) {
        sqlx::query(
            "INSERT INTO conversations.decisions
                    (id, workspace_id, message_id, conversation_id, question, source,
                     strategy, value, model, acted, state_version)
             SELECT gen_random_uuid(), workspace_id, id, conversation_id, 'suspicious',
                    'model', 'laya', 'false'::jsonb, 'laya-test', false, 1
               FROM conversations.messages
             ON CONFLICT DO NOTHING",
        )
        .execute(db)
        .await
        .unwrap();
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn a_trusted_sender_s_images_show_only_when_it_is_authenticated(db: PgPool) {
        let kit = Kit::new(db.clone()).await;
        let (workspace, inbox, sam) = kit.sams_inbox().await;
        let recorded = json!({"changes": [
            newsletter("trusted-1", "October at Brightloop", Some(
                "mx.google.com; dkim=pass header.i=@brightloop.example header.s=s1; \
                 dmarc=pass (p=REJECT sp=REJECT dis=NONE) header.from=brightloop.example")),
            // Signed by someone else, no DMARC: a spoofed From.
            newsletter("spoofed-1", "Your Brightloop account", Some(
                "mx.google.com; dkim=pass header.i=@spoofer.example header.s=s1; \
                 spf=pass smtp.mailfrom=bounce@spoofer.example")),
            newsletter("dmarc-1", "Brightloop invoice", Some(
                "mx.google.com; spf=pass smtp.mailfrom=bounce@spoofer.example; \
                 dmarc=fail (p=REJECT sp=REJECT dis=REJECT) header.from=brightloop.example")),
            newsletter("noauth-1", "Brightloop news", None),
        ]});
        let batch = dev::record(&kit.state, workspace, inbox, &recorded)
            .await
            .unwrap();
        dev::submit(&kit.state, workspace, &batch).await.unwrap();
        kit.run_jobs().await;
        let rules = format!("/api/v1/w/{workspace}/inboxes/{inbox}/rules");
        let trust = |address: &str| {
            json!({
                "conditions": [{"field": "sender_address", "value": address}],
                "actions": [{"action": "show_images"}],
            })
        };
        // No sender the member has no mail from.
        let stranger = kit
            .post(&sam, &rules, Some(trust("stranger@elsewhere.example")))
            .await;
        assert_eq!(stranger.status, StatusCode::BAD_REQUEST);
        // Before: held back.
        assert_eq!(
            body_of(&kit, &sam, workspace, "trusted-1").await["images_shown"],
            false
        );

        let created = kit
            .post(&sam, &rules, Some(trust("News@Brightloop.example")))
            .await;
        assert_eq!(created.status, StatusCode::CREATED, "{}", created.json);
        let rule = created.json["rule"]["id"].as_str().unwrap().to_owned();
        // It changes no mail: nothing to apply.
        let runs: i64 = sqlx::query_scalar("SELECT count(*) FROM conversations.rule_runs")
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(runs, 0);
        // Not judged yet (no `suspicious` answer): nothing is trusted.
        assert_eq!(
            body_of(&kit, &sam, workspace, "trusted-1").await["images_trusted"],
            false
        );
        // Judged not suspicious, every one of them.
        judged_not_suspicious(&db).await;

        let shown = body_of(&kit, &sam, workspace, "trusted-1").await;
        assert_eq!(shown["images_trusted"], true);
        assert_eq!(shown["images_shown"], true);
        let html = shown["html"].as_str().unwrap();
        assert!(
            html.contains("https://cdn.brightloop.example/banner.png"),
            "{html}"
        );
        // The tracking pixel stays removed for a trusted sender.
        assert!(!html.contains("t.brightloop.example"), "{html}");
        // A spoofed From, a failed DMARC check and no results: never.
        for id in ["spoofed-1", "dmarc-1", "noauth-1"] {
            let held = body_of(&kit, &sam, workspace, id).await;
            assert_eq!(held["images_trusted"], false, "{id}");
            assert_eq!(held["images_shown"], false, "{id}");
            assert!(
                !held["html"]
                    .as_str()
                    .unwrap()
                    .contains("cdn.brightloop.example"),
                "{id}"
            );
        }

        // Removed, "Show images" is back.
        let removed = kit
            .delete(&sam, &format!("/api/v1/w/{workspace}/rules/{rule}"))
            .await;
        assert!(removed.status.is_success(), "{}", removed.status);
        let after = body_of(&kit, &sam, workspace, "trusted-1").await;
        assert_eq!(after["images_trusted"], false);
        assert_eq!(after["images_shown"], false);
    }

    /// Brightloop's newsletter four ways: authenticated, a spoofed From, a
    /// failed DMARC check and no results.
    async fn newsletters(
        kit: &Kit,
        workspace: campbooks_kernel::tenancy::WorkspaceId,
        inbox: crate::model::InboxId,
    ) {
        let recorded = json!({"changes": [
            newsletter("trusted-1", "October at Brightloop", Some(
                "mx.google.com; dkim=pass header.i=@brightloop.example header.s=s1; \
                 dmarc=pass (p=REJECT sp=REJECT dis=NONE) header.from=brightloop.example")),
            newsletter("spoofed-1", "Your Brightloop account", Some(
                "mx.google.com; dkim=pass header.i=@spoofer.example header.s=s1; \
                 spf=pass smtp.mailfrom=bounce@spoofer.example")),
            newsletter("dmarc-1", "Brightloop invoice", Some(
                "mx.google.com; spf=pass smtp.mailfrom=bounce@spoofer.example; \
                 dmarc=fail (p=REJECT sp=REJECT dis=REJECT) header.from=brightloop.example")),
            newsletter("noauth-1", "Brightloop news", None),
        ]});
        let batch = dev::record(&kit.state, workspace, inbox, &recorded)
            .await
            .unwrap();
        dev::submit(&kit.state, workspace, &batch).await.unwrap();
        kit.run_jobs().await;
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn the_reader_is_offered_only_what_can_take_effect(db: PgPool) {
        let kit = Kit::new(db.clone()).await;
        let (workspace, inbox, sam) = kit.sams_inbox().await;
        newsletters(&kit, workspace, inbox).await;
        let body = |id: &'static str| body_of(&kit, &sam, workspace, id);
        // Untrusted and authenticated: offered, with its domain.
        let fresh = body("trusted-1").await;
        assert_eq!(fresh["images_offer"], true, "{fresh}");
        assert_eq!(fresh["images_domain"], "brightloop.example");
        assert!(fresh.get("images_held").is_none(), "{fresh}");
        // Where trusting could never take effect: not offered, and why.
        for (id, why) in [
            ("spoofed-1", "checks_fail"),
            ("dmarc-1", "checks_fail"),
            ("noauth-1", "cannot_authenticate"),
        ] {
            let held = body(id).await;
            assert_eq!(held["images_offer"], false, "{id}");
            assert_eq!(held["images_held"], why, "{id}");
        }

        let rules = format!("/api/v1/w/{workspace}/inboxes/{inbox}/rules");
        let trust = |field: &str, value: &str| {
            json!({
                "conditions": [{"field": field, "value": value}],
                "actions": [{"action": "show_images"}],
            })
        };
        let created = kit
            .post(
                &sam,
                &rules,
                Some(trust("sender_address", "news@brightloop.example")),
            )
            .await;
        assert_eq!(created.status, StatusCode::CREATED, "{}", created.json);
        // Taught: no second offer, and the images wait for Scout's check.
        let waiting = body("trusted-1").await;
        assert_eq!(waiting["images_offer"], false);
        assert_eq!(waiting["images_held"], "not_checked");
        // The same sender twice is refused; a whole domain is not (it is
        // broader), and then covers the address.
        let twice = kit
            .post(
                &sam,
                &rules,
                Some(trust("sender_address", "news@brightloop.example")),
            )
            .await;
        assert_eq!(twice.status, StatusCode::CONFLICT, "{}", twice.json);
        assert_eq!(twice.json["code"], "images_already_trusted");
        let domain = kit
            .post(
                &sam,
                &rules,
                Some(trust("sender_domain", "brightloop.example")),
            )
            .await;
        assert_eq!(domain.status, StatusCode::CREATED, "{}", domain.json);
        let covered = kit
            .post(
                &sam,
                &rules,
                Some(trust("sender_domain", "brightloop.example")),
            )
            .await;
        assert_eq!(covered.status, StatusCode::CONFLICT);

        // Judged not suspicious: trusted. A stored `suspicious = true` then
        // keeps them back again, and says so.
        judged_not_suspicious(&db).await;
        assert_eq!(body("trusted-1").await["images_trusted"], true);
        sqlx::query(
            "INSERT INTO conversations.decisions
                    (id, workspace_id, message_id, conversation_id, question, source,
                     strategy, value, model, acted, state_version)
             SELECT gen_random_uuid(), workspace_id, id, conversation_id, 'suspicious',
                    'rules', 'rules', 'true'::jsonb, 'rules', false, 1
               FROM conversations.messages WHERE provider_message_id = 'trusted-1'",
        )
        .execute(&db)
        .await
        .unwrap();
        let review = body("trusted-1").await;
        assert_eq!(review["images_trusted"], false);
        assert_eq!(review["images_shown"], false);
        assert_eq!(review["images_held"], "under_review");
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn an_image_rule_kept_from_before_for_a_shared_platform_never_trusts(db: PgPool) {
        use campbooks_kernel::db::BeginFor as _;

        let kit = Kit::new(db.clone()).await;
        let (workspace, inbox, sam) = kit.sams_inbox().await;
        // A newsletter on a platform anyone sends from, authenticated by it.
        let mut message = newsletter(
            "platform-1",
            "Brightloop weekly",
            Some(
                "mx.google.com; dkim=pass header.i=@substack.com header.s=s1; \
                 dmarc=pass (p=REJECT sp=REJECT dis=NONE) header.from=substack.com",
            ),
        );
        message["message"]["from"] =
            json!({"name": "Brightloop", "address": "brightloop@substack.com"});
        let batch = dev::record(&kit.state, workspace, inbox, &json!({"changes": [message]}))
            .await
            .unwrap();
        dev::submit(&kit.state, workspace, &batch).await.unwrap();
        kit.run_jobs().await;
        judged_not_suspicious(&db).await;
        // An active, signed rule trusting anyone at the platform, stored as
        // if from before the platform was listed (the API refuses it now).
        let rule = sqlx::types::Uuid::now_v7();
        let mut tx = kit.state.db.begin_for(workspace).await.unwrap();
        sqlx::query(
            "INSERT INTO conversations.rules
                 (id, workspace_id, inbox_id, teaching_id, conditions, actions, specificity,
                  origin, state, confirmed_at)
             VALUES ($1, $2, $3, $4,
                     '[{\"field\": \"sender_domain\", \"value\": \"substack.com\"}]'::jsonb,
                     '[{\"action\": \"show_images\"}]'::jsonb, 1, 'action', 'active', now())",
        )
        .bind(rule)
        .bind(workspace.as_uuid())
        .bind(inbox.as_uuid())
        .bind(sqlx::types::Uuid::now_v7())
        .execute(&mut *tx)
        .await
        .unwrap();
        crate::domain::rules::binding::sign(
            &mut tx,
            &kit.state.binding,
            crate::model::RuleId::from_uuid(rule),
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();

        let body = body_of(&kit, &sam, workspace, "platform-1").await;
        assert_eq!(body["images_trusted"], false, "{body}");
        assert_eq!(body["images_shown"], false);
        assert_eq!(body["images_held"], "rule_too_broad");
        // The address alone may still be trusted.
        assert_eq!(body["images_offer"], true);
        assert!(body["images_domain"].is_null());
        // Removed, it does not come back.
        let removed = kit
            .delete(&sam, &format!("/api/v1/w/{workspace}/rules/{rule}"))
            .await;
        assert!(removed.status.is_success(), "{}", removed.json);
        let restored = kit
            .post(
                &sam,
                &format!("/api/v1/w/{workspace}/rules/{rule}/restore"),
                Some(json!({})),
            )
            .await;
        assert_eq!(
            restored.status,
            StatusCode::BAD_REQUEST,
            "{}",
            restored.json
        );
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn words_alone_are_the_text_or_the_html_read_as_text(db: PgPool) {
        // campbooks-cloud#250: what a reply or forward quotes when
        // `conversations.get` could not read a message's HTML in time.
        let kit = Kit::new(db).await;
        let (workspace, inbox, sam) = kit.sams_inbox().await;
        kit.ingest_value(
            workspace,
            inbox,
            &json!({"changes": [{"op": "upsert", "message": {
                "provider_message_id": "html-only-1",
                "from": {"name": "Quay Office", "address": "office@quay-moorings.example"},
                "to": [{"address": "sam@example.com"}],
                "subject": "Your berth for next season",
                "sent_at": "2026-10-13T06:30:00Z",
                "direction": "inbound",
                "html": "<head><meta charset=utf-8><title>Quay Office</title>\
                         <p>Your berth at the harbourlight pontoon is kept.</p>",
                "mailbox": {"in_inbox": true},
                "read": false, "starred": false
            }}]}),
        )
        .await;
        let words = |id: &'static str| {
            let kit = &kit;
            let sam = &sam;
            async move {
                let (conversation, message): (String, String) = sqlx::query_as(
                    "SELECT conversation_id::text, id::text FROM conversations.messages
                      WHERE provider_message_id = $1",
                )
                .bind(id)
                .fetch_one(&kit.db)
                .await
                .unwrap();
                kit.get(
                    sam,
                    &format!(
                        "/api/v1/w/{workspace}/conversations/{conversation}/messages/{message}/body?words=true"
                    ),
                )
                .await
            }
        };
        let html_only = words("html-only-1").await;
        assert_eq!(html_only.status, StatusCode::OK, "{}", html_only.json);
        let text = html_only.json["text"].as_str().unwrap_or_default();
        assert!(
            text.contains("harbourlight pontoon") && !text.contains("Quay Office"),
            "{text:?}"
        );
        assert!(html_only.json["html"].is_null());
        assert_eq!(html_only.json["truncated"], false);
        // HTML past the 256 KiB read: its words come, marked cut, so a quote
        // of them says it is not whole.
        let long = "<p>The harbourlight pontoon schedule, week by week.</p>".repeat(6_000);
        kit.ingest_value(
            workspace,
            inbox,
            &json!({"changes": [{"op": "upsert", "message": {
                "provider_message_id": "html-long-1",
                "from": {"name": "Quay Office", "address": "office@quay-moorings.example"},
                "to": [{"address": "sam@example.com"}],
                "subject": "The season's schedule",
                "sent_at": "2026-10-13T07:30:00Z",
                "direction": "inbound",
                "html": long,
                "mailbox": {"in_inbox": true},
                "read": false, "starred": false
            }}]}),
        )
        .await;
        let cut = words("html-long-1").await;
        assert_eq!(cut.status, StatusCode::OK, "{}", cut.json);
        assert!(
            cut.json["text"]
                .as_str()
                .unwrap_or_default()
                .starts_with("The harbourlight pontoon schedule"),
            "{}",
            cut.json["text"]
        );
        assert_eq!(cut.json["truncated"], true);
        // A message with a text part answers with it.
        let plain = words("en-1").await;
        assert!(
            plain.json["text"]
                .as_str()
                .unwrap_or_default()
                .contains("by Friday 10 October"),
            "{}",
            plain.json
        );
        // Who may not read it gets nothing.
        let (other, _) = kit.member("leo", workspace);
        let (conversation, message): (String, String) = sqlx::query_as(
            "SELECT conversation_id::text, id::text FROM conversations.messages
              WHERE provider_message_id = 'html-only-1'",
        )
        .fetch_one(&kit.db)
        .await
        .unwrap();
        let hidden = kit
            .get(
                &other,
                &format!(
                    "/api/v1/w/{workspace}/conversations/{conversation}/messages/{message}/body?words=true"
                ),
            )
            .await;
        assert_eq!(hidden.status, StatusCode::NOT_FOUND, "{}", hidden.json);
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn spam_never_loads_remote_images_not_when_asked_nor_trusted(db: PgPool) {
        let kit = Kit::new(db.clone()).await;
        let (workspace, inbox, sam) = kit.sams_inbox().await;
        kit.ingest_value(
            workspace,
            inbox,
            &json!({"changes": [newsletter("spam-1", "October at Brightloop", Some(
                "mx.google.com; dkim=pass header.i=@brightloop.example header.s=s1; \
                 dmarc=pass (p=REJECT sp=REJECT dis=NONE) header.from=brightloop.example"))]}),
        )
        .await;
        let rules = format!("/api/v1/w/{workspace}/inboxes/{inbox}/rules");
        let created = kit
            .post(
                &sam,
                &rules,
                Some(json!({
                    "conditions": [{"field": "sender_address", "value": "news@brightloop.example"}],
                    "actions": [{"action": "show_images"}],
                })),
            )
            .await;
        assert_eq!(created.status, StatusCode::CREATED, "{}", created.json);
        judged_not_suspicious(&db).await;
        assert_eq!(
            body_of(&kit, &sam, workspace, "spam-1").await["images_trusted"],
            true
        );

        let conversation = kit.conversation_of("spam-1").await;
        let reported = kit
            .post(
                &sam,
                &format!("/api/v1/w/{workspace}/conversations/{conversation}/spam"),
                None,
            )
            .await;
        assert_eq!(reported.status, StatusCode::OK);
        let held = body_of(&kit, &sam, workspace, "spam-1").await;
        assert_eq!(held["images_trusted"], false);
        assert_eq!(held["images_shown"], false);
        assert_eq!(held["images_offer"], false);
        assert_eq!(held["images_held"], "spam");
        // Asked for, still held back.
        let message = kit.message_of("spam-1").await;
        let asked = kit
            .get(
                &sam,
                &format!(
                    "/api/v1/w/{workspace}/conversations/{conversation}/messages/{message}/body?show_images=true"
                ),
            )
            .await;
        assert_eq!(asked.json["images_shown"], false);
        assert!(
            !asked.json["html"]
                .as_str()
                .unwrap()
                .contains("cdn.brightloop.example")
        );

        // Not spam: trusted again.
        kit.post(
            &sam,
            &format!("/api/v1/w/{workspace}/conversations/{conversation}/not-spam"),
            None,
        )
        .await;
        assert_eq!(
            body_of(&kit, &sam, workspace, "spam-1").await["images_shown"],
            true
        );
    }
}
