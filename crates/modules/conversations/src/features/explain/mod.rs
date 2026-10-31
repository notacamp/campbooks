//! `POST /api/v1/w/{workspace_id}/conversations/{conversation_id}/explanation`
//! — a member asks Scout to explain a thread (campbooks-cloud#407).
//!
//! Only on the member's own request. It checks, as reading the conversation
//! does, that the member may read it now, then that it can be explained (not
//! in spam or the trash, its inbox not kept read-only, a message, no
//! phishing warning: 409 `not_explainable`) and that a `complete` role may
//! read mail here (503), and then, with the member's row locked:
//!
//! - a request for the same thread state and language that is being written
//!   answers with it (202), and one that is ready answers with it (200): no
//!   second call, whatever the clicks or tabs;
//! - anything else sets the row `writing` (keeping an earlier explanation as
//!   it was), counts one of the member's 20 requests an hour, queues the job
//!   `conversations.explain_thread`, and answers 202.
//!
//! It never waits for the model: the card shows "Scout is reading…" until the
//! job stores the answer and the member's own change feed says so.

use std::time::Duration;

use axum::{Extension, Json, extract::State, http::StatusCode};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, RecordRef, Result, Risk,
    authz::Action,
    db::BeginFor as _,
    extract,
    i18n::Locale,
    jobs::{self, EnqueueOptions},
    rate_limit::{Limit, RateKey},
    tenancy::{Workspace, WorkspaceId},
};

use crate::{
    domain::{
        explanation::{
            self, ExplanationState as RowState, ExplanationView, Row, SELECT, SELECT_LOCKED,
            WRITING_MINUTES,
        },
        paths::ConversationPath,
        resources,
    },
    events::CONVERSATION_RECORD,
    features::explain_thread::{ExplainThread, ExplainThreadPayload},
    model::ConversationId,
    scopes::ConversationsRead,
};

/// Explanations a member may ask for per hour.
const LIMIT: Limit = Limit::new("conversations.explain", 20, Duration::from_hours(1));
/// Longest name the prompt is told for the member, in characters.
const NAME_CHARS: usize = 200;

/// Whether a `complete` role may read mail here: explanations are offered and
/// accepted only then.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Explainer {
    /// A `complete` role is configured and may receive mail.
    pub available: bool,
}

/// Ask Scout to explain a thread.
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/conversations/{conversation_id}/explanation",
    operation_id = "conversations.explain",
    tag = "conversations",
    security(("bearer" = ["conversations:read"])),
    // A command: it spends a model call and the member's hourly allowance.
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low).records(["conversations.conversation"]).access("conversations.conversation", campbooks_kernel::authz::Action::Read)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("conversation_id" = ConversationId, Path, description = "The conversation."),
    ),
    responses(
        (status = OK, description = "The member's explanation of this thread and language is ready: nothing was asked of the model.",
            body = ExplanationView),
        (status = ACCEPTED, description = "The explanation is being written (asked now, or already asked): the answer arrives as a change to the conversation, or read it with `conversations.get_explanation`.",
            body = ExplanationView),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope.", body = Problem,
            content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such conversation, or you cannot see it.",
            body = Problem, content_type = "application/problem+json"),
        (status = TOO_MANY_REQUESTS, description = "Too many explanations asked for this hour.",
            body = Problem, content_type = "application/problem+json"),
        (status = CONFLICT, description = "This thread cannot be explained (`not_explainable`): it is in spam or the trash, its inbox is kept read-only, it has no messages, or it raised a phishing warning.",
            body = Problem, content_type = "application/problem+json"),
        (status = SERVICE_UNAVAILABLE, description = "No language model may read mail here (`explain_unavailable`).",
            body = Problem, content_type = "application/problem+json"),
    )
)]
#[allow(clippy::too_many_lines)]
pub async fn handler(
    State(state): State<AppState>,
    Extension(explainer): Extension<Explainer>,
    workspace: Workspace<ConversationsRead>,
    locale: Locale,
    extract::Path(path): extract::Path<ConversationPath>,
) -> Result<(StatusCode, Json<ExplanationView>)> {
    let conversation = path.conversation_id;
    let access = &workspace.access;
    let language = locale.tag();
    let mut tx = state.db.begin_for(access.workspace).await?;
    let allowed = resources::authorize(
        &state,
        &mut tx,
        access,
        Action::Read,
        &RecordRef::new(CONVERSATION_RECORD, &conversation),
    )
    .await;
    let (member, inboxes) =
        match explanation::member_sees(&mut tx, access, conversation, allowed).await {
            Ok(seen) => seen,
            Err(Error::NotFound) => {
                // Their explanation went with their access.
                tx.commit().await?;
                return Err(Error::NotFound);
            }
            Err(other) => return Err(other),
        };
    if !explanation::explainable(&mut tx, conversation).await? {
        return Err(Error::Conflict(crate::MESSAGES.reason("not_explainable")));
    }
    if !explainer.available {
        return Err(Error::Unavailable(
            crate::MESSAGES.reason("explain_unavailable"),
        ));
    }
    let current = explanation::current(&mut tx, conversation)
        .await?
        .ok_or_else(|| Error::Conflict(crate::MESSAGES.reason("not_explainable")))?;
    let version = current.version();
    let reader: Option<String> = workspace
        .principal
        .name
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(|name| name.chars().take(NAME_CHARS).collect());

    // The member's row for this conversation, locked: a second click or tab
    // waits here and then finds the first one's row.
    let created = sqlx::query(
        "INSERT INTO conversations.explanations
             (workspace_id, member_id, conversation_id, state, asked_version, asked_locale, reader)
         VALUES ($1, $2, $3, 'writing', $4, $5, $6)
         ON CONFLICT DO NOTHING",
    )
    .bind(access.workspace.as_uuid())
    .bind(member.as_uuid())
    .bind(conversation)
    .bind(&version)
    .bind(language)
    .bind(reader.as_deref())
    .execute(&mut *tx)
    .await?
    .rows_affected()
        == 1;
    let row: Row = sqlx::query_as(SELECT_LOCKED)
        .bind(access.workspace.as_uuid())
        .bind(member.as_uuid())
        .bind(conversation)
        .fetch_one(&mut *tx)
        .await?;

    let same_ask = row.asked_version == version && row.asked_locale == language;
    let same_kept = row.thread_version.as_deref() == Some(version.as_str())
        && row.locale.as_deref() == Some(language);
    let writing = !created && row.state == RowState::Writing.as_str() && same_ask;
    let ready = !created && row.state == RowState::Ready.as_str() && same_kept;
    // A request that never answered (its job failed or was lost) is asked
    // again after a while, instead of being waited for for ever.
    let lost: bool = writing && {
        sqlx::query_scalar("SELECT $1 < now() - make_interval(mins => $2)")
            .bind(row.updated_at)
            .bind(WRITING_MINUTES)
            .fetch_one(&mut *tx)
            .await?
    };
    if ready || (writing && !lost) {
        let status = if ready {
            StatusCode::OK
        } else {
            StatusCode::ACCEPTED
        };
        let view = explanation::view(
            &mut tx,
            &row,
            Some(&current),
            language,
            &inboxes,
            conversation,
        )
        .await?;
        tx.commit().await?;
        return Ok((status, Json(view)));
    }

    // A call will be made, unless a job for this member and thread is
    // pending already (it takes the row as it is now).
    let queued = jobs::enqueue_to::<ExplainThread>(
        &mut tx,
        &ExplainThreadPayload {
            member,
            conversation,
        },
        EnqueueOptions::for_workspace(access.workspace.to_string()).unique(format!(
            "conversations.explain_thread:{member}:{conversation}"
        )),
    )
    .await?;
    if matches!(queued, jobs::Enqueued::Created(_)) {
        // One of the member's twenty an hour. The transaction is dropped
        // (the row and the job with it) if they are over.
        let who = format!("{}:{member}", access.workspace);
        state
            .rate_limits
            .check(&state.db, &LIMIT, &RateKey::text(&who))
            .await?;
        if !created {
            sqlx::query(
                "UPDATE conversations.explanations
                    SET state = 'writing', failure = NULL, asked_version = $4,
                        asked_locale = $5, reader = $6, updated_at = now()
                  WHERE workspace_id = $1 AND member_id = $2 AND conversation_id = $3",
            )
            .bind(access.workspace.as_uuid())
            .bind(member.as_uuid())
            .bind(conversation)
            .bind(&version)
            .bind(language)
            .bind(reader.as_deref())
            .execute(&mut *tx)
            .await?;
        }
    }
    let row: Row = sqlx::query_as(SELECT)
        .bind(access.workspace.as_uuid())
        .bind(member.as_uuid())
        .bind(conversation)
        .fetch_one(&mut *tx)
        .await?;
    let view = explanation::view(
        &mut tx,
        &row,
        Some(&current),
        language,
        &inboxes,
        conversation,
    )
    .await?;
    tx.commit().await?;
    Ok((StatusCode::ACCEPTED, Json(view)))
}
