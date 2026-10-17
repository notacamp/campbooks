//! Teach once (campbooks-cloud#125): after a member acts on one
//! conversation, which rules would do the same to the mail that looks like
//! it, each with a preview, and the one to offer first.
//!
//! Likenesses are deterministic and said in plain words: the sender, the
//! list, or the sender (or its domain) with the subject's template. A person
//! the inbox writes to gets no default, and is never offered a rule that
//! hides their mail; suspicious mail never starts a suggestion; a member who
//! turned a suggestion down does not see it again for 30 days, and never
//! after the second time.

#[cfg(test)]
mod tests;

use axum::{Json, extract::State, http::StatusCode};
use campbooks_ai::decide::{
    signals::email::{Envelope, Party, is_machine_sender, registrable_domain},
    taught::is_free_mail,
};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, RecordRef, Result, Risk,
    authz::Action,
    db::BeginFor as _,
    extract,
    tenancy::{Access, Workspace, WorkspaceId},
};
use serde::{Deserialize, Serialize};
use sqlx::PgConnection;
use utoipa::{IntoParams, ToSchema};

use crate::{
    domain::{
        paths::ConversationPath,
        resources,
        rules::{self, Candidate, RuleAction, RuleCondition, SurfaceWhen},
    },
    events::CONVERSATION_RECORD,
    features::rules::{PREVIEW_DEADLINE, RulePreview, compute_preview, may_teach},
    model::{ConversationId, InboxId, LabelId, Stream},
    scopes::{ConversationsRead, ConversationsWrite},
};

/// How long a turned-down suggestion stays hidden, in days.
pub const DISMISS_DAYS: i32 = 30;
/// Lookalikes the default needs (besides the one acted on) to be offered.
pub const MIN_LOOKALIKES: u32 = 2;

/// What the member just did to the conversation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TaughtAction {
    /// Moved it to a stream (give `stream`).
    MoveToStream,
    /// Archived it (Done in the inbox).
    Archive,
    /// Marked it read.
    MarkRead,
    /// Put a label on it (give `label`).
    AddLabel,
    /// Marked its need Not needed.
    NotNeeded,
    /// Reported it as spam or phishing ("Report and delete"): offers to
    /// report mail like it as it arrives ("Never see emails like this
    /// again?"), suspicious as it is.
    Report,
    /// Said its bill is already paid: offers to mark that sender's bills
    /// paid from now on.
    Paid,
}

/// The action to suggest a rule for.
#[derive(Debug, Clone, Copy, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct SuggestQuery {
    /// What the member did.
    #[param(inline)]
    pub action: TaughtAction,
    /// The stream, for `move_to_stream`.
    pub stream: Option<Stream>,
    /// The label, for `add_label`.
    pub label: Option<LabelId>,
}

/// How a suggestion picks the mail that looks the same.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Likeness {
    /// Everything from the sender.
    Sender,
    /// Everything from the mailing list.
    List,
    /// The sender's mail with this subject's template.
    SenderTemplate,
    /// The domain's mail with this subject's template.
    DomainTemplate,
    /// Everything from the sender's domain.
    Domain,
    /// Mail that says it is from the sender's domain but fails its sender
    /// checks (someone pretending to be it).
    Impersonation,
}

/// One rule the member could make.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct RuleSuggestion {
    /// How it picks the mail.
    pub likeness: Likeness,
    /// Its conditions.
    pub conditions: Vec<RuleCondition>,
    /// Its actions.
    pub actions: Vec<RuleAction>,
    /// What it would do now.
    pub preview: RulePreview,
    /// Names it when turning it down.
    pub key: String,
}

/// Why there is nothing to offer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum NoSuggestion {
    /// It looks suspicious.
    Suspicious,
    /// The inbox writes to this sender: their mail is never hidden by a rule.
    Correspondent,
    /// Too few conversations look like it.
    TooFew,
    /// The member turned it down.
    Dismissed,
    /// Nothing to teach from this action here.
    NotTeachable,
}

/// The rules the member could make from what they just did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct RuleSuggestions {
    /// Broad to narrow; empty when there is nothing to offer.
    pub suggestions: Vec<RuleSuggestion>,
    /// The one to offer first, by index; `None` lets the member choose.
    pub default: Option<u32>,
    /// Other members use the inbox: a rule acts for them too.
    pub shared: bool,
    /// Why there is nothing to offer, when there is not.
    pub none: Option<NoSuggestion>,
}

impl RuleSuggestions {
    fn nothing(why: NoSuggestion, shared: bool) -> Self {
        Self {
            suggestions: Vec::new(),
            default: None,
            shared,
            none: Some(why),
        }
    }
}

/// Suggest rules that do to lookalikes what the member just did.
#[utoipa::path(
    get,
    path = "/api/v1/w/{workspace_id}/conversations/{conversation_id}/rule-suggestions",
    operation_id = "conversations.suggest_rules",
    tag = "conversations",
    security(("bearer" = ["conversations:read"])),
    extensions(("x-campbooks" = json!(Operation::query().assistant().records(["conversations.rule", "conversations.conversation"]).access("conversations.inbox", campbooks_kernel::authz::Action::Write)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("conversation_id" = ConversationId, Path, description = "The conversation acted on."),
        SuggestQuery,
    ),
    responses(
        (status = OK, description = "The suggestions.", body = RuleSuggestions),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, or you may only read this inbox.",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such conversation, or you cannot see it.",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn suggest(
    State(state): State<AppState>,
    workspace: Workspace<ConversationsRead>,
    extract::Path(path): extract::Path<ConversationPath>,
    extract::Query(query): extract::Query<SuggestQuery>,
) -> Result<Json<RuleSuggestions>> {
    let access = &workspace.access;
    let mut tx = crate::features::rules::begin_read(&state, access.workspace).await?;
    let inbox = teachable_inbox(&state, &mut tx, access, path.conversation_id).await?;
    let out = suggestions(&mut tx, access, inbox, path.conversation_id, query).await?;
    tx.commit().await?;
    Ok(Json(out))
}

/// The conversation's inbox if the caller may teach there: 404 when they
/// cannot see the conversation, 403 `inbox_read_only` when they only read its
/// inbox or it is kept read-only (the catalogue's `write` on the inbox).
async fn teachable_inbox(
    state: &AppState,
    conn: &mut PgConnection,
    access: &Access,
    id: ConversationId,
) -> Result<InboxId> {
    resources::authorize(
        state,
        conn,
        access,
        Action::Read,
        &RecordRef::new(CONVERSATION_RECORD, &id),
    )
    .await?;
    let inbox = sqlx::query_scalar!(
        r#"SELECT inbox_id AS "inbox: InboxId" FROM conversations.conversations WHERE id = $1"#,
        id as ConversationId,
    )
    .fetch_optional(&mut *conn)
    .await?
    .ok_or(Error::NotFound)?;
    may_teach(state, conn, access, inbox).await?;
    Ok(inbox)
}

async fn suggestions(
    conn: &mut PgConnection,
    access: &Access,
    inbox: InboxId,
    id: ConversationId,
    query: SuggestQuery,
) -> Result<RuleSuggestions> {
    // One deadline for every likeness's preview together.
    let deadline = std::time::Instant::now() + PREVIEW_DEADLINE;
    let shared = rules::is_shared(conn, inbox).await?;
    let Some(candidate) = rules::candidate(conn, id).await? else {
        return Ok(RuleSuggestions::nothing(NoSuggestion::NotTeachable, shared));
    };
    // Reporting is about suspicious mail: the one offer it may start.
    if candidate.suspicious && query.action != TaughtAction::Report {
        return Ok(RuleSuggestions::nothing(NoSuggestion::Suspicious, shared));
    }
    let Some(actions) = actions_for(conn, id, query).await? else {
        return Ok(RuleSuggestions::nothing(NoSuggestion::NotTeachable, shared));
    };
    let correspondent = rules::corresponds(conn, inbox, &candidate.sender_address).await?;
    // "Paid" hides a question, never the mail: a correspondent's bills too.
    if correspondent && rules::hides(&actions) && query.action != TaughtAction::Paid {
        return Ok(RuleSuggestions::nothing(
            NoSuggestion::Correspondent,
            shared,
        ));
    }
    // About mail still to come: offered even with no lookalike yet.
    let ahead = matches!(query.action, TaughtAction::Report | TaughtAction::Paid);
    if ahead {
        return ahead_suggestions(conn, access, inbox, &candidate, &actions, shared, deadline)
            .await;
    }

    let mut out = Vec::new();
    for (likeness, conditions) in likenesses(&candidate) {
        if rules::validate(&conditions, &actions).is_err() {
            continue;
        }
        let preview = compute_preview(conn, inbox, (&conditions, &actions), deadline).await?;
        // A likeness that would change nothing is no choice at all.
        if preview.would_change == 0 {
            continue;
        }
        let key = suggestion_key(likeness, &conditions, &actions);
        out.push(RuleSuggestion {
            likeness,
            conditions,
            actions: actions.clone(),
            preview,
            key,
        });
    }
    let machine = is_machine_sender(&envelope(&candidate));
    let has_list = out.iter().any(|s| s.likeness == Likeness::List);
    let default = if correspondent {
        None
    } else if machine {
        out.iter().position(|s| s.likeness == Likeness::Sender)
    } else if has_list {
        out.iter().position(|s| s.likeness == Likeness::List)
    } else {
        out.iter().position(|s| {
            matches!(
                s.likeness,
                Likeness::SenderTemplate | Likeness::DomainTemplate
            )
        })
    };
    let worth = |s: &RuleSuggestion| s.preview.would_change >= MIN_LOOKALIKES;
    let offered = match default {
        Some(i) => out
            .get(i)
            .is_some_and(|s| worth(s) || s.likeness == Likeness::List),
        None => out.iter().any(worth),
    };
    if !offered {
        return Ok(RuleSuggestions::nothing(NoSuggestion::TooFew, shared));
    }
    if dismissed(conn, access, inbox, &out, default).await? {
        return Ok(RuleSuggestions::nothing(NoSuggestion::Dismissed, shared));
    }
    Ok(RuleSuggestions {
        suggestions: out,
        default: default.and_then(|i| u32::try_from(i).ok()),
        shared,
        none: None,
    })
}

/// The offers about mail still to come (after Report and Already paid): the
/// sender, or someone pretending to be its domain, or the whole domain,
/// each with what it would do to the mail already here, the first the
/// default.
async fn ahead_suggestions(
    conn: &mut PgConnection,
    access: &Access,
    inbox: InboxId,
    candidate: &Candidate,
    actions: &[RuleAction],
    shared: bool,
    deadline: std::time::Instant,
) -> Result<RuleSuggestions> {
    let mut out = Vec::new();
    for (likeness, conditions) in ahead_likenesses(candidate, rules::bins(actions)) {
        if rules::validate(&conditions, actions).is_err() {
            continue;
        }
        let preview = compute_preview(conn, inbox, (&conditions, actions), deadline).await?;
        let key = suggestion_key(likeness, &conditions, actions);
        out.push(RuleSuggestion {
            likeness,
            conditions,
            actions: actions.to_vec(),
            preview,
            key,
        });
    }
    if out.is_empty() {
        return Ok(RuleSuggestions::nothing(NoSuggestion::NotTeachable, shared));
    }
    let default = Some(0);
    if dismissed(conn, access, inbox, &out, default).await? {
        return Ok(RuleSuggestions::nothing(NoSuggestion::Dismissed, shared));
    }
    Ok(RuleSuggestions {
        suggestions: out,
        default: Some(0),
        shared,
        none: None,
    })
}

/// Broad to narrow for mail still to come. When the provider says the
/// sender failed its checks, the From domain is the one being pretended
/// to: the offer is "pretending to be it" first, never the whole domain
/// (its real mail would go too).
fn ahead_likenesses(candidate: &Candidate, bins: bool) -> Vec<(Likeness, Vec<RuleCondition>)> {
    let sender = candidate.sender_address.clone();
    let domain = registrable_domain(&sender);
    let own_domain = !domain.is_empty() && !is_free_mail(&domain);
    let by_sender = (Likeness::Sender, vec![RuleCondition::SenderAddress(sender)]);
    // Mail pretending to be a sender is only ever offered to the bin: never
    // marked paid, never filed.
    if bins && candidate.auth_failed() && own_domain {
        return vec![
            (
                Likeness::Impersonation,
                vec![
                    RuleCondition::SenderDomain(domain),
                    RuleCondition::SenderChecksFail,
                ],
            ),
            by_sender,
        ];
    }
    let mut out = vec![by_sender];
    if own_domain {
        out.push((Likeness::Domain, vec![RuleCondition::SenderDomain(domain)]));
    }
    out
}

/// Whether the member turned the offer down: its default's key, or with no
/// default every likeness offered, since "Just this one" turns all of them
/// down.
async fn dismissed(
    conn: &mut PgConnection,
    access: &Access,
    inbox: InboxId,
    out: &[RuleSuggestion],
    default: Option<usize>,
) -> Result<bool> {
    let keys: Vec<String> = match default.and_then(|i| out.get(i)) {
        Some(first) => vec![first.key.clone()],
        None => out.iter().map(|s| s.key.clone()).collect(),
    };
    if let Some(user) = access.user {
        let dismissed = sqlx::query_scalar!(
            r#"SELECT coalesce(bool_or(times >= 2
                                       OR dismissed_at > now() - make_interval(days => $4)),
                               false) AS "hidden!"
                 FROM conversations.rule_dismissals
                WHERE inbox_id = $1 AND user_id = $2 AND suggestion_key = ANY($3)"#,
            inbox as InboxId,
            user.as_uuid(),
            &keys,
            DISMISS_DAYS,
        )
        .fetch_one(&mut *conn)
        .await?;
        return Ok(dismissed);
    }
    Ok(false)
}

/// The actions that repeat what the member did, or `None` when there is
/// nothing to teach from it.
async fn actions_for(
    conn: &mut PgConnection,
    id: ConversationId,
    query: SuggestQuery,
) -> Result<Option<Vec<RuleAction>>> {
    Ok(match query.action {
        TaughtAction::MoveToStream => query
            .stream
            .map(|stream| vec![RuleAction::MoveToStream { stream }]),
        TaughtAction::Archive => Some(vec![RuleAction::Archive]),
        TaughtAction::MarkRead => Some(vec![RuleAction::MarkRead]),
        TaughtAction::AddLabel => query
            .label
            .map(|label| vec![RuleAction::AddLabel { label }]),
        TaughtAction::NotNeeded => {
            let need = sqlx::query!(
                "SELECT reason, question FROM conversations.needs
                  WHERE conversation_id = $1 AND verdict = 'not_needed'",
                id as ConversationId,
            )
            .fetch_optional(conn)
            .await?;
            need.and_then(|n| match (n.reason.as_str(), n.question.as_str()) {
                (_, "suspicious") => None,
                ("pay", _) => Some(vec![RuleAction::Surface {
                    value: SurfaceWhen::Never,
                }]),
                (_, "needs_reply") => Some(vec![RuleAction::NeedsReply { value: false }]),
                (_, "asks_action") => Some(vec![RuleAction::AsksAction { value: false }]),
                _ => None,
            })
        }
        TaughtAction::Report => {
            let reported = sqlx::query_scalar!(
                r#"SELECT EXISTS (SELECT 1 FROM conversations.messages
                                   WHERE conversation_id = $1 AND in_spam) AS "reported!""#,
                id as ConversationId,
            )
            .fetch_one(conn)
            .await?;
            reported.then(|| vec![RuleAction::Spam])
        }
        TaughtAction::Paid => {
            let paid = sqlx::query_scalar!(
                r#"SELECT EXISTS (SELECT 1 FROM conversations.needs
                                   WHERE conversation_id = $1 AND reason = 'pay'
                                     AND verdict = 'already_paid') AS "paid!""#,
                id as ConversationId,
            )
            .fetch_one(conn)
            .await?;
            paid.then(|| vec![RuleAction::MarkPaid])
        }
    })
}

/// The likenesses of `candidate`, broad to narrow.
fn likenesses(candidate: &Candidate) -> Vec<(Likeness, Vec<RuleCondition>)> {
    let sender = candidate.sender_address.clone();
    let mut out = vec![(
        Likeness::Sender,
        vec![RuleCondition::SenderAddress(sender.clone())],
    )];
    if let Some(list) = candidate
        .headers
        .list_id
        .as_deref()
        .filter(|l| !l.trim().is_empty())
    {
        out.push((
            Likeness::List,
            vec![RuleCondition::ListId(list.trim().to_owned())],
        ));
    }
    if let Some(template) = &candidate.subject_template {
        out.push((
            Likeness::SenderTemplate,
            vec![
                RuleCondition::SenderAddress(sender.clone()),
                RuleCondition::SubjectTemplate(template.clone()),
            ],
        ));
        let local = sender.split('@').next().unwrap_or_default();
        let domain = registrable_domain(&sender);
        if local.chars().any(|c| c.is_ascii_digit()) && !is_free_mail(&domain) {
            out.push((
                Likeness::DomainTemplate,
                vec![
                    RuleCondition::SenderDomain(domain),
                    RuleCondition::SubjectTemplate(template.clone()),
                ],
            ));
        }
    }
    out
}

fn envelope(candidate: &Candidate) -> Envelope {
    let mut headers = std::collections::BTreeMap::new();
    for (name, value) in [
        ("List-Id", &candidate.headers.list_id),
        ("List-Unsubscribe", &candidate.headers.list_unsubscribe),
        ("Precedence", &candidate.headers.precedence),
        ("Auto-Submitted", &candidate.headers.auto_submitted),
    ] {
        if let Some(value) = value {
            headers.insert(name.to_owned(), value.clone());
        }
    }
    Envelope {
        from: Party {
            name: String::new(),
            address: candidate.sender_address.clone(),
        },
        subject: candidate.subject.clone(),
        headers,
        ..Envelope::default()
    }
}

/// A stable name for a suggestion: its likeness, conditions and actions.
fn suggestion_key(
    likeness: Likeness,
    conditions: &[RuleCondition],
    actions: &[RuleAction],
) -> String {
    let conditions = serde_json::to_string(conditions).unwrap_or_default();
    let actions = serde_json::to_string(actions).unwrap_or_default();
    let likeness = serde_json::to_string(&likeness).unwrap_or_default();
    format!("{likeness}|{conditions}|{actions}")
}

/// The suggestion turned down.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct DismissSuggestion {
    /// Its `key`.
    pub key: String,
}

/// Turn a suggestion down ("Just this one"): it is not offered again for 30
/// days, and never after the second time.
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/conversations/{conversation_id}/rule-suggestions/dismiss",
    operation_id = "conversations.dismiss_rule_suggestion",
    tag = "conversations",
    security(("bearer" = ["conversations:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .records(["conversations.rule"]).access("conversations.inbox", campbooks_kernel::authz::Action::Write)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("conversation_id" = ConversationId, Path, description = "The conversation acted on."),
    ),
    request_body = DismissSuggestion,
    responses(
        (status = NO_CONTENT, description = "It will not be offered again for now."),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, or you may only read this inbox.",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such conversation, or you cannot see it.",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn dismiss(
    State(state): State<AppState>,
    workspace: Workspace<ConversationsWrite>,
    extract::Path(path): extract::Path<ConversationPath>,
    extract::Json(body): extract::Json<DismissSuggestion>,
) -> Result<StatusCode> {
    let access = &workspace.access;
    let mut tx = state.db.begin_for(access.workspace).await?;
    let inbox = teachable_inbox(&state, &mut tx, access, path.conversation_id).await?;
    if let Some(user) = access.user {
        sqlx::query!(
            r#"INSERT INTO conversations.rule_dismissals
                   (workspace_id, inbox_id, user_id, suggestion_key)
               VALUES ($1, $2, $3, $4)
               ON CONFLICT (inbox_id, user_id, suggestion_key) DO UPDATE
                  SET times = conversations.rule_dismissals.times + 1, dismissed_at = now()"#,
            access.workspace.as_uuid(),
            inbox as InboxId,
            user.as_uuid(),
            body.key.chars().take(2000).collect::<String>(),
        )
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}
