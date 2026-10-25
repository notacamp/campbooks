//! `POST …/conversations/{conversation_id}/need/resolve`, `/need/confirm` and
//! `/need/reopen` — a member's verdict on what a conversation needs.
//!
//! Done clears the need for everyone who can see the inbox (owner,
//! 2026-10-13); Not needed also stores a correction, the labelled example the
//! decide evals learn from (campbooks-cloud#59); confirming a question turns
//! it into a need Campbooks stands behind, and is stored as a correction too.
//! Reopen undoes either, withdrawing the correction.
//!
//! A "Still needed?" question (an old undated ask, campbooks-cloud#275) is
//! about time, not the model's reading: Yes keeps it as the member's own,
//! Not needed clears it, and neither stores a correction.
//!
//! A Pay need asks "Is it paid?" (campbooks-cloud#167): Already paid clears
//! it as done and files nothing (yet); Still to pay keeps it as a need
//! Campbooks stands behind, stored as a correction confirming the money
//! answer. Both are only for a Pay need (409 `not_a_payment`).
//!
//! A question about a reply or an action (`needs_reply`, `asks_action`) on
//! mail that went to other people may be answered "someone else has the
//! move" (campbooks-cloud#372): Not needed with `wait_on`, the address of
//! one of the email's people (409 `not_a_party` if it is not). It files two
//! corrections, `next_move` (from `recipient`, or what the model said, to
//! `someone_else`) and `awaited` (to the option naming that person), and the
//! conversation waits on that person from then on (`needs::Need::waiting_on`),
//! a wait Campbooks stands behind. Reopen withdraws both and brings the
//! question back.
//!
//! A wait on someone (campbooks-cloud#293) is resolved the same way: Done is
//! "Heard from them" and files nothing; Not needed is "Not waiting", stored
//! as a correction of the model's `next_move` answer (from what it said to
//! [`NOT_WAITING`]).

use axum::{Extension, Json, extract::State};
use campbooks_ai::decide::signals::{
    email::{Party, triage},
    facts,
    moves::{self, NextMove},
};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, Result, Risk,
    db::BeginFor as _,
    extract,
    tenancy::{Access, Workspace, WorkspaceId},
};
use serde::{Deserialize, Serialize};
use sqlx::{Postgres, Transaction};
use utoipa::ToSchema;

use crate::{
    domain::paths::ConversationPath,
    domain::{
        access::lock_conversation,
        conversation::{ConversationSummary, summary},
        needs::{self, NeedSettings},
        tracked,
    },
    model::{
        ConversationId, CorrectionId, DecisionId, MessageId, NeedBasis, NeedQuestion, NeedReason,
        NeedVerdict,
    },
    scopes::ConversationsWrite,
    tracking,
};

#[cfg(test)]
mod tests;
#[cfg(test)]
mod unsure_tests;
#[cfg(test)]
mod waiting_tests;

/// What a correction of a Pay need's money answer says when the member
/// marks it Not needed: no payment is owed (paid already, or no bill).
pub const NOT_OWED: &str = "not_owed";

/// What a correction of a wait's `next_move` answer says when the member
/// marks it Not waiting: the model's choice was wrong (which option was
/// right is the labeller's to say).
pub const NOT_WAITING: &str = "not_waiting";

/// What a correction of `next_move` says when the member answers that
/// someone else on the email has the move.
const SOMEONE_ELSE: &str = "someone_else";

/// How a member resolves a need.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    /// It is done (replied by phone, paid, signed…).
    Done,
    /// It was never needed: Campbooks got it wrong.
    NotNeeded,
    /// A Pay need: it is already paid (cleared as done; nothing filed yet).
    AlreadyPaid,
    /// A Pay need: it is still to pay (kept, as a need Campbooks stands
    /// behind).
    StillToPay,
}

/// A member's verdict on a need.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct Resolve {
    /// Done, Not needed, or for a Pay need Already paid or Still to pay.
    pub outcome: Outcome,
    /// With Not needed on a question about a reply or an action: the address
    /// of the person on the email who has the move. The conversation then
    /// waits on them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(example = "ana@example.com")]
    pub wait_on: Option<String>,
    /// The message the need the member saw comes from (`NeedView.message_id`):
    /// when the open need comes from another one, nothing is recorded (409
    /// `need_changed`).
    pub message_id: MessageId,
}

/// Yes to a question.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct Confirm {
    /// The message the question the member saw comes from.
    pub message_id: MessageId,
}

/// Resolve what a conversation needs: done, not needed, already paid or still to pay.
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/conversations/{conversation_id}/need/resolve",
    operation_id = "conversations.resolve_need",
    tag = "conversations",
    security(("bearer" = ["conversations:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .undo("conversations.reopen_need")
        .assistant()
        .records(["conversations.conversation", "conversations.correction"]).access("conversations.conversation", campbooks_kernel::authz::Action::Write)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("conversation_id" = ConversationId, Path, description = "The conversation."),
    ),
    request_body = Resolve,
    responses(
        (status = OK, description = "The conversation afterwards.", body = ConversationSummary),
        (status = BAD_REQUEST, description = "Not an outcome.", body = Problem,
            content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, or you may only read this inbox (`inbox_read_only`).",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such conversation, or you cannot see it.",
            body = Problem, content_type = "application/problem+json"),
        (status = CONFLICT, description = "It needs nothing (`no_open_need`), not what you saw any more (`need_changed`), a paid answer for a need that is not Pay (`not_a_payment`), or a person to wait on who is not on the email (`not_a_party`).",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn resolve(
    State(state): State<AppState>,
    Extension(settings): Extension<NeedSettings>,
    workspace: Workspace<ConversationsWrite>,
    extract::Path(path): extract::Path<ConversationPath>,
    extract::Json(body): extract::Json<Resolve>,
) -> Result<Json<ConversationSummary>> {
    let verdict = match body.outcome {
        Outcome::Done => NeedVerdict::Done,
        Outcome::NotNeeded => NeedVerdict::NotNeeded,
        Outcome::AlreadyPaid => NeedVerdict::AlreadyPaid,
        Outcome::StillToPay => NeedVerdict::StillToPay,
    };
    judge(
        &state,
        &settings,
        &workspace.access,
        path.conversation_id,
        verdict,
        body.message_id,
        body.wait_on.as_deref(),
    )
    .await
    .map(Json)
}

/// Answer yes to a question Campbooks put (a Review need it was unsure of).
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/conversations/{conversation_id}/need/confirm",
    operation_id = "conversations.confirm_need",
    tag = "conversations",
    security(("bearer" = ["conversations:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .undo("conversations.reopen_need")
        .assistant()
        .records(["conversations.conversation", "conversations.correction"]).access("conversations.conversation", campbooks_kernel::authz::Action::Write)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("conversation_id" = ConversationId, Path, description = "The conversation."),
    ),
    request_body = Confirm,
    responses(
        (status = OK, description = "The conversation afterwards.", body = ConversationSummary),
        (status = BAD_REQUEST, description = "No message id.", body = Problem,
            content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, or you may only read this inbox (`inbox_read_only`).",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such conversation, or you cannot see it.",
            body = Problem, content_type = "application/problem+json"),
        (status = CONFLICT, description = "No question to answer (`no_open_need`), or not the one you saw (`need_changed`).",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn confirm(
    State(state): State<AppState>,
    Extension(settings): Extension<NeedSettings>,
    workspace: Workspace<ConversationsWrite>,
    extract::Path(path): extract::Path<ConversationPath>,
    extract::Json(body): extract::Json<Confirm>,
) -> Result<Json<ConversationSummary>> {
    judge(
        &state,
        &settings,
        &workspace.access,
        path.conversation_id,
        NeedVerdict::Confirmed,
        body.message_id,
        None,
    )
    .await
    .map(Json)
}

/// Take back Done, Not needed or a confirmation: the need is as it was.
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/conversations/{conversation_id}/need/reopen",
    operation_id = "conversations.reopen_need",
    tag = "conversations",
    security(("bearer" = ["conversations:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .assistant()
        .records(["conversations.conversation", "conversations.correction"]).access("conversations.conversation", campbooks_kernel::authz::Action::Write)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("conversation_id" = ConversationId, Path, description = "The conversation."),
    ),
    responses(
        (status = OK, description = "The conversation afterwards.", body = ConversationSummary),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, or you may only read this inbox (`inbox_read_only`).",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such conversation, or you cannot see it.",
            body = Problem, content_type = "application/problem+json"),
        (status = CONFLICT, description = "Nothing to bring back (`nothing_to_reopen`).",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn reopen(
    State(state): State<AppState>,
    Extension(settings): Extension<NeedSettings>,
    workspace: Workspace<ConversationsWrite>,
    extract::Path(path): extract::Path<ConversationPath>,
) -> Result<Json<ConversationSummary>> {
    let access = &workspace.access;
    let id = path.conversation_id;
    let mut tx = state.db.begin_for(access.workspace).await?;
    lock_conversation(&state, &mut tx, access, id).await?;
    // Done or Already paid over an earlier confirmation (Still to pay, then
    // Already paid): Undo goes back to the confirmation, which keeps its
    // correction.
    let restored = sqlx::query(
        "UPDATE conversations.needs
            SET verdict = CASE WHEN question = 'money' THEN 'still_to_pay' ELSE 'confirmed' END,
                updated_at = now()
          WHERE conversation_id = $1 AND verdict_message_id = message_id
            AND verdict IN ('done', 'already_paid') AND verdict_correction_id IS NOT NULL",
    )
    .bind(id.as_uuid())
    .execute(&mut *tx)
    .await?
    .rows_affected()
        > 0;
    if restored {
        needs::refresh(
            &mut tx,
            &state.events,
            &access.workspace.to_string(),
            id,
            &settings,
        )
        .await?;
        let summary = summary(&mut tx, access.user, id).await?;
        track(&state, &mut tx, access, Judged::Reopened, &summary).await?;
        tx.commit().await?;
        return Ok(Json(summary));
    }
    // The second correction of "someone else has the move", read before the
    // verdict goes (a runtime query: the column was added since the `.sqlx`
    // cache).
    let extra: Option<sqlx::types::Uuid> = sqlx::query_scalar::<_, Option<sqlx::types::Uuid>>(
        "SELECT verdict_extra_correction_id FROM conversations.needs
          WHERE conversation_id = $1 AND verdict_message_id = message_id",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?
    .flatten();
    let withdrawn = sqlx::query_scalar!(
        r#"
        UPDATE conversations.needs n
           SET verdict = NULL, verdict_message_id = NULL, verdict_by = NULL,
               verdict_correction_id = NULL, updated_at = now()
          FROM (SELECT conversation_id, verdict_correction_id FROM conversations.needs
                 WHERE conversation_id = $1 AND verdict IS NOT NULL
                   AND verdict_message_id = message_id FOR UPDATE) old
         WHERE n.conversation_id = old.conversation_id
        RETURNING old.verdict_correction_id AS "correction: CorrectionId"
        "#,
        id as ConversationId,
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| Error::Conflict(crate::MESSAGES.reason("nothing_to_reopen")))?;
    if let Some(correction) = withdrawn {
        withdraw(&mut tx, correction).await?;
    }
    if let Some(extra) = extra {
        withdraw(&mut tx, CorrectionId::from_uuid(extra)).await?;
    }
    sqlx::query(
        "UPDATE conversations.needs
            SET verdict_awaited_name = NULL, verdict_awaited_address = NULL,
                verdict_extra_correction_id = NULL
          WHERE conversation_id = $1",
    )
    .bind(id)
    .execute(&mut *tx)
    .await?;
    needs::refresh(
        &mut tx,
        &state.events,
        &access.workspace.to_string(),
        id,
        &settings,
    )
    .await?;
    let summary = summary(&mut tx, access.user, id).await?;
    track(&state, &mut tx, access, Judged::Reopened, &summary).await?;
    tx.commit().await?;
    Ok(Json(summary))
}

/// What a member did to a conversation's need, as it is tracked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Judged {
    Resolved,
    Confirmed,
    Reopened,
}

/// Track what the member did to the conversation's need (ADR 0020,
/// campbooks-cloud#325), with the conversation and its need afterwards,
/// read in the transaction of the change.
async fn track(
    state: &AppState,
    tx: &mut Transaction<'_, Postgres>,
    access: &Access,
    judged: Judged,
    after: &ConversationSummary,
) -> Result<()> {
    let workspace = access.workspace;
    let conversation = tracked::conversation(after);
    let need = tracked::need(tx, after.id).await?;
    match judged {
        Judged::Resolved => {
            state
                .tracker
                .track(
                    tx,
                    &tracking::NeedResolved {
                        workspace,
                        conversation,
                        need,
                    },
                )
                .await?;
        }
        Judged::Confirmed => {
            state
                .tracker
                .track(
                    tx,
                    &tracking::NeedConfirmed {
                        workspace,
                        conversation,
                        need,
                    },
                )
                .await?;
        }
        Judged::Reopened => {
            state
                .tracker
                .track(
                    tx,
                    &tracking::NeedReopened {
                        workspace,
                        conversation,
                        need,
                    },
                )
                .await?;
        }
    }
    Ok(())
}

/// Withdraw `correction`: it no longer says what the member thinks.
async fn withdraw(tx: &mut Transaction<'_, Postgres>, correction: CorrectionId) -> Result<()> {
    sqlx::query!(
        "UPDATE conversations.corrections SET withdrawn_at = now() WHERE id = $1",
        correction as CorrectionId,
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// Record `verdict` on conversation `id`'s open need.
#[allow(clippy::too_many_lines)]
async fn judge(
    state: &AppState,
    settings: &NeedSettings,
    access: &Access,
    id: ConversationId,
    verdict: NeedVerdict,
    seen: MessageId,
    wait_on: Option<&str>,
) -> Result<ConversationSummary> {
    let mut tx = state.db.begin_for(access.workspace).await?;
    lock_conversation(state, &mut tx, access, id).await?;
    let need = sqlx::query!(
        r#"
        SELECT message_id AS "message: MessageId", reason, question, basis, trust,
               decision_id AS "decision: DecisionId"
          FROM conversations.needs
         WHERE conversation_id = $1 AND state = 'open' FOR UPDATE
        "#,
        id as ConversationId,
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| Error::Conflict(crate::MESSAGES.reason("no_open_need")))?;
    // Not the need the member saw (a newer message replaced it meanwhile):
    // record nothing rather than a wrong correction.
    if need.message != seen {
        return Err(Error::Conflict(crate::MESSAGES.reason("need_changed")));
    }
    // Someone else has the move: found among the email's own people.
    let wait = match wait_on {
        Some(address) => Some(
            wait_target(
                &mut tx,
                (need.message, need.question.as_str(), need.trust.as_str()),
                verdict,
                address,
            )
            .await?,
        ),
        None => None,
    };
    let paid_answer = matches!(verdict, NeedVerdict::AlreadyPaid | NeedVerdict::StillToPay);
    if paid_answer && need.reason != NeedReason::Pay.as_str() {
        return Err(Error::Conflict(crate::MESSAGES.reason("not_a_payment")));
    }
    // Still to pay again, on a bill already confirmed: nothing to change.
    if verdict == NeedVerdict::StillToPay && need.trust != "check" {
        let summary = summary(&mut tx, access.user, id).await?;
        tx.commit().await?;
        return Ok(summary);
    }
    // Only a question can be confirmed.
    if verdict == NeedVerdict::Confirmed && need.trust != "check" {
        return Err(Error::Conflict(crate::MESSAGES.reason("no_open_need")));
    }
    // A confirmation the member gave earlier on this message (Still to pay,
    // then Already paid): kept with the later verdict, so Undo brings the
    // confirmed need back rather than the question.
    let earlier: Option<Option<sqlx::types::Uuid>> = sqlx::query_scalar(
        "SELECT verdict_correction_id FROM conversations.needs
          WHERE conversation_id = $1 AND verdict_message_id = message_id
            AND verdict IN ('confirmed', 'still_to_pay')",
    )
    .bind(id.as_uuid())
    .fetch_optional(&mut *tx)
    .await?;
    let earlier_correction = earlier.flatten().map(CorrectionId::from_uuid);
    // The money question is a choice: its answer behind a Pay need is
    // "owed"; "not owed" is what Not needed says of it.
    // The move question is a choice too: its answer behind a wait is who
    // had the move ("sender", "someone_else").
    let moved: Option<String> = match need.decision {
        Some(decision) if need.question == NeedQuestion::NextMove.as_str() => {
            sqlx::query_scalar::<_, Option<String>>(
                "SELECT value #>> '{}' FROM conversations.decisions WHERE id = $1",
            )
            .bind(decision)
            .fetch_optional(&mut *tx)
            .await?
            .flatten()
        }
        _ => None,
    };
    let (yes, no) = if need.question == NeedQuestion::Money.as_str() {
        (triage::OWED, NOT_OWED)
    } else if let Some(moved) = moved.as_deref() {
        (moved, NOT_WAITING)
    } else {
        ("true", "false")
    };
    // "Worth a look" is the member's own rule at work, not an answer to
    // correct: whatever the verdict, it files nothing (Seen is its Done).
    let taught_highlight = need.question == NeedQuestion::WorthALook.as_str();
    let to = match verdict {
        _ if taught_highlight => None,
        NeedVerdict::Done | NeedVerdict::AlreadyPaid => None,
        NeedVerdict::NotNeeded => Some(no),
        NeedVerdict::Confirmed | NeedVerdict::StillToPay => Some(yes),
    };
    let mut extra = None;
    let correction = match (to, &wait) {
        // Someone else has the move: `next_move` and `awaited` are what the
        // member corrected, not whether the email needs a reply.
        (Some(_), Some(wait)) => {
            let first = correct(
                &mut tx,
                access,
                id,
                need.message,
                Correction {
                    question: moves::question::NEXT_MOVE,
                    from: &wait.moved_from,
                    to: SOMEONE_ELSE,
                    overridden: &need.basis,
                    decision: wait.moved_decision,
                },
            )
            .await?;
            extra = Some(
                correct(
                    &mut tx,
                    access,
                    id,
                    need.message,
                    Correction {
                        question: moves::question::AWAITED,
                        from: &wait.awaited_from,
                        to: &wait.label,
                        overridden: &need.basis,
                        decision: wait.awaited_decision,
                    },
                )
                .await?,
            );
            Some(first)
        }
        (Some(_), None) if about_time(&need.question) => None,
        (Some(to), None) => Some(
            correct(
                &mut tx,
                access,
                id,
                need.message,
                Correction {
                    question: &need.question,
                    from: yes,
                    to,
                    overridden: &need.basis,
                    decision: need.decision,
                },
            )
            .await?,
        ),
        // Done or Already paid file nothing: an earlier confirmation stays.
        (None, _) => earlier_correction,
    };
    // Not needed after Still to pay (or a confirmation) on this message: the
    // earlier "owed" correction no longer holds, so the labels never say both.
    if verdict == NeedVerdict::NotNeeded
        && let Some(earlier) = earlier_correction
    {
        withdraw(&mut tx, earlier).await?;
    }
    sqlx::query!(
        r#"
        UPDATE conversations.needs
           SET verdict = $2, verdict_message_id = message_id, verdict_by = $3,
               verdict_correction_id = $4, updated_at = now()
         WHERE conversation_id = $1
        "#,
        id as ConversationId,
        verdict.as_str(),
        access.user.map(|u| *u.as_uuid()),
        correction as Option<CorrectionId>,
    )
    .execute(&mut *tx)
    .await?;
    // Who the member said has the move, kept with the verdict (or cleared,
    // for any other answer). A runtime query, so the columns added since need
    // no entry in the `.sqlx` offline cache.
    sqlx::query(
        "UPDATE conversations.needs
            SET verdict_awaited_name = $2, verdict_awaited_address = $3,
                verdict_extra_correction_id = $4
          WHERE conversation_id = $1",
    )
    .bind(id)
    .bind(wait.as_ref().map(|w| w.party.name.trim().to_owned()))
    .bind(wait.as_ref().map(|w| w.party.address.trim().to_owned()))
    .bind(extra.map(|c| *c.as_uuid()))
    .execute(&mut *tx)
    .await?;
    needs::refresh(
        &mut tx,
        &state.events,
        &access.workspace.to_string(),
        id,
        settings,
    )
    .await?;
    let summary = summary(&mut tx, access.user, id).await?;
    let judged = if verdict == NeedVerdict::Confirmed {
        Judged::Confirmed
    } else {
        Judged::Resolved
    };
    track(state, &mut tx, access, judged, &summary).await?;
    tx.commit().await?;
    Ok(summary)
}

/// The person a member said has the move, with what to file about it.
struct WaitTarget {
    /// Who, as the email's headers gave them.
    party: Party,
    /// The `awaited` option that names them.
    label: String,
    /// What `next_move` said before (`recipient` if it was not asked).
    moved_from: String,
    /// The `next_move` answer, if one was stored.
    moved_decision: Option<DecisionId>,
    /// What `awaited` said before (`none` if it was not asked).
    awaited_from: String,
    /// The `awaited` answer, if one was stored.
    awaited_decision: Option<DecisionId>,
}

/// Find `address` among the people of the email a question about a reply or
/// an action is about, and the stored answers to correct. Only Not needed on
/// such a question may carry one (409 `no_open_need`), and only a person the
/// email's headers name and facts let be awaited (409 `not_a_party`).
async fn wait_target(
    tx: &mut Transaction<'_, Postgres>,
    (message, question, trust): (MessageId, &str, &str),
    verdict: NeedVerdict,
    address: &str,
) -> Result<WaitTarget> {
    let asks = question == NeedQuestion::NeedsReply.as_str()
        || question == NeedQuestion::AsksAction.as_str();
    if verdict != NeedVerdict::NotNeeded || trust != "check" || !asks {
        return Err(Error::Conflict(crate::MESSAGES.reason("no_open_need")));
    }
    let not_a_party = || Error::Conflict(crate::MESSAGES.reason("not_a_party"));
    let people = needs::people(tx, message).await?.ok_or_else(not_a_party)?;
    let label = moves::awaited_label(&people, address).ok_or_else(not_a_party)?;
    let party = facts::awaitable(&people)
        .into_iter()
        .find(|p| p.address.trim().eq_ignore_ascii_case(address.trim()))
        .ok_or_else(not_a_party)?;
    let stored = needs::answers(tx, message).await?;
    let model = |id: &str| {
        stored
            .iter()
            .find(|a| a.question == id && a.basis == NeedBasis::Model)
    };
    let said = |id: &str, otherwise: &str| {
        model(id)
            .and_then(|a| a.value.as_str())
            .unwrap_or(otherwise)
            .to_owned()
    };
    Ok(WaitTarget {
        party,
        label,
        moved_from: said(moves::question::NEXT_MOVE, NextMove::Recipient.as_str()),
        moved_decision: model(moves::question::NEXT_MOVE).map(|a| a.id),
        awaited_from: said(moves::question::AWAITED, moves::question::NONE),
        awaited_decision: model(moves::question::AWAITED).map(|a| a.id),
    })
}

/// Whether `question` is "Still needed?": it asks about time, not about
/// the model's reading of the mail, so neither answer is a labelled example
/// (campbooks-cloud#275).
fn about_time(question: &str) -> bool {
    [NeedQuestion::StillToDo, NeedQuestion::StillNeedsReply]
        .iter()
        .any(|q| question == q.as_str())
}

/// A correction of the answer behind a need.
struct Correction<'a> {
    question: &'a str,
    from: &'a str,
    to: &'a str,
    overridden: &'a str,
    decision: Option<DecisionId>,
}

/// Store a correction of the answer behind a need: the labelled example.
async fn correct(
    tx: &mut Transaction<'_, Postgres>,
    access: &Access,
    conversation: ConversationId,
    message: MessageId,
    correction: Correction<'_>,
) -> Result<CorrectionId> {
    let id = CorrectionId::new();
    let decisions: Vec<DecisionId> = correction.decision.into_iter().collect();
    sqlx::query!(
        r#"
        INSERT INTO conversations.corrections
            (id, workspace_id, conversation_id, message_id, question, from_value, to_value,
             overridden_source, decision_ids, corrected_by)
        VALUES ($1, $2, $3, $4, $5, $10, $6, $7, $8, $9)
        "#,
        id as CorrectionId,
        access.workspace.as_uuid(),
        conversation as ConversationId,
        message as MessageId,
        correction.question,
        correction.to,
        correction.overridden,
        &decisions as &[DecisionId],
        access.user.map(|u| *u.as_uuid()),
        correction.from,
    )
    .execute(&mut **tx)
    .await?;
    Ok(id)
}
