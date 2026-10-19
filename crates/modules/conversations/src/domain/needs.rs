//! What a conversation needs from its inbox's members (attention,
//! campbooks-cloud#39).
//!
//! At most one need per conversation, kept in `conversations.needs`:
//!
//! - **Raised** ([`raise`], pure) from the decide pass's answers about the
//!   newest inbound message that is not in spam or the trash: `pay` when the
//!   money answer says a payment is still owed, always as a question ("Is it
//!   paid?", campbooks-cloud#167: Campbooks cannot see the bank); `reply` or
//!   `do` from answers the policy let act; `review` for a suspicious message
//!   (a warning) or, when a model answers, for a suggestion put to the
//!   member as a question. Mail sorted to Receipts needs nothing but a
//!   payment still owed: a receipt or a payment confirmation needs nothing.
//!   The facts only ever say "no" (ADR 0018), so a sure need comes from the
//!   model; a facts "no" still wins over a model's "yes".
//! - **Settled** ([`settle`], pure) against what happened since: a reply
//!   clears Reply and Do, an archive clears Reply and Review, the trash or
//!   spam (the conversation's, or the need's own message's) clears
//!   everything, and a member's verdict (Done, Not needed, a confirmed
//!   question, Already paid, Still to pay) wins.
//! - **Refreshed** ([`refresh`]) wherever those inputs change, publishing
//!   `conversations.need_changed` with a growing revision when the need did.
//!
//! Mail text is never an instruction here. A need carries no due date until
//! one is extracted from the mail (a follow-up of ADR 0018); answers stored
//! before then may carry the sender's sentence, only ever shown, quoted.

use campbooks_ai::decide::signals::email::triage;
use campbooks_kernel::{Error, EventBus, Result, tenancy::UserId};
use chrono::{DateTime, NaiveDate, TimeDelta, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgConnection, Postgres, Transaction};
use utoipa::ToSchema;

use super::{conversation::ConversationSummary, decide::Source, policy::Policy};
use crate::{
    events::{NeedChanged, NeedFacts},
    model::{
        Address, ClearCause, ConversationId, DecisionId, InboxId, MessageId, NeedBasis,
        NeedQuestion, NeedReason, NeedTrust, NeedVerdict,
    },
};

/// The question a taught rule's `surface` answer is stored under.
pub const SURFACE: &str = "surface";
/// The question a taught rule's "Worth a look" answer is stored under
/// (campbooks-cloud#274): the member asked to see each message from a sender
/// until they have opened it once.
pub const WORTH_A_LOOK: &str = "worth_a_look";
/// The `urgency` level meaning "today".
const TODAY_LEVEL: f64 = 2.0;
/// The stream of automated money mail: only a payment still owed needs
/// anything there.
const RECEIPTS: &str = "receipts";
/// The stream of people writing personally.
const PRIMARY: &str = "primary";
/// The stream of list mail, which never needs anything of the recipient.
const NEWSLETTERS: &str = "newsletters";

/// How needs are raised: the window of mail that may raise one, and the
/// policy (which stream answers count, and its floor for questions).
#[derive(Debug, Clone)]
pub struct NeedSettings {
    /// Only inbound messages sent within this long ago raise a need.
    pub window: TimeDelta,
    /// Which answers may act.
    pub policy: Policy,
    /// Whether a need that opens queues Scout's summary of its email
    /// (`conversations.summarize_need`): only when the `complete` role may
    /// receive mail on this server.
    pub summaries: bool,
}

impl NeedSettings {
    /// A window of `days`, under `policy`.
    pub fn new(days: u32, policy: &Policy) -> Self {
        Self {
            window: TimeDelta::days(i64::from(days)),
            policy: policy.clone(),
            summaries: false,
        }
    }

    /// The same settings, queueing a summary for each need that opens when
    /// `on`.
    #[must_use]
    pub const fn with_summaries(mut self, on: bool) -> Self {
        self.summaries = on;
        self
    }

    /// The confidence from which a model suggestion becomes a question.
    pub const fn check_floor(&self) -> f64 {
        self.policy.check_floor()
    }
}

/// One stored answer, as [`raise`] reads it.
#[derive(Debug, Clone, PartialEq)]
pub struct Answer {
    /// The stored decision.
    pub id: DecisionId,
    /// The question id (`needs_reply`, …).
    pub question: String,
    /// `rules` or `model`.
    pub basis: NeedBasis,
    /// The stored value: a boolean, a level or a label.
    pub value: serde_json::Value,
    /// Its confidence, when known.
    pub confidence: Option<f64>,
    /// Whether the policy let it act.
    pub acted: bool,
    /// Its reasons.
    pub reasons: Vec<StoredReason>,
}

/// A stored reason.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct StoredReason {
    /// `<strategy>.<rule>`.
    pub rule: String,
    /// What it matched.
    #[serde(default)]
    pub evidence: String,
    /// The sender's sentence holding the evidence.
    #[serde(default)]
    pub sentence: Option<String>,
}

impl Answer {
    fn is_true(&self) -> bool {
        self.value.as_bool() == Some(true)
    }

    fn level(&self) -> Option<f64> {
        self.value.as_f64()
    }

    /// The decide pass's source of this answer (a taught rule's answers
    /// are stored acted, so the policy is never asked about them).
    fn source(&self) -> Source {
        match self.basis {
            NeedBasis::Rules | NeedBasis::Taught => Source::Rules,
            NeedBasis::Model | NeedBasis::Member => Source::Model,
        }
    }
}

/// A need, before its state is known.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Need {
    /// The inbound message it comes from.
    pub message: MessageId,
    /// Why.
    pub reason: NeedReason,
    /// How much Campbooks stands behind it.
    pub trust: NeedTrust,
    /// Whose answer raised it.
    pub basis: NeedBasis,
    /// The question that raised it.
    pub question: NeedQuestion,
    /// The answer that explains it.
    pub decision: Option<DecisionId>,
    /// When it is due.
    pub due_on: Option<NaiveDate>,
    /// Whether it is for today.
    pub urgent: bool,
}

/// The answer to `question` the policy let act, when it says yes: the
/// rules' answer when they acted, else the model's. An acted "no" from the
/// rules wins over a model's acted "yes".
fn accepted<'a>(answers: &'a [Answer], question: &str) -> Option<&'a Answer> {
    answers
        .iter()
        .filter(|a| a.question == question && a.acted)
        .min_by_key(|a| a.basis != NeedBasis::Rules)
        .filter(|a| a.is_true())
}

/// The stream the message was sorted to by an answer the policy accepts:
/// the stream answer acted (it placed the conversation), or the policy
/// accepts it (a later message of a conversation already placed). The
/// facts' answer wins over the model's, as for every question.
fn sorted_stream<'a>(answers: &'a [Answer], policy: &Policy) -> Option<&'a str> {
    answers
        .iter()
        .filter(|a| {
            a.question == triage::STREAM
                && (a.acted
                    || policy
                        .switch(triage::STREAM, a.source())
                        .accepts_confidence(a.confidence))
        })
        .min_by_key(|a| a.basis != NeedBasis::Rules)
        .and_then(|a| a.value.as_str())
}

/// Whether the message may need a reply or an action of the recipient,
/// from the model's own answers (never from the text): a person writing
/// personally may; list mail never does; automated mail (or mail no answer
/// sorted) only when an accepted answer gives it a time limit. A calendar
/// invite or a support notice reads like an ask, and the model says so;
/// what the automated asks that matter share is a deadline
/// (campbooks-cloud#144).
fn may_ask(stream: Option<&str>, answers: &[Answer]) -> bool {
    match stream {
        Some(PRIMARY) => true,
        Some(NEWSLETTERS) => false,
        _ => accepted(answers, triage::HAS_DEADLINE).is_some(),
    }
}

/// Whether an answer the policy let act says "no" to `question`: the
/// question is settled, and no suggestion reopens it.
fn settled_no(answers: &[Answer], question: &str) -> bool {
    answers
        .iter()
        .any(|a| a.question == question && a.acted && !a.is_true())
}

/// The answer saying the message asks for a payment still owed, when one
/// counts: the policy let it act, or it is the model's, at least
/// [`NeedSettings::check_floor`] sure (a Pay need is a question either
/// way). An acted answer saying otherwise wins.
fn owed<'a>(answers: &'a [Answer], settings: &NeedSettings) -> Option<&'a Answer> {
    let money = |a: &&Answer| a.question == triage::MONEY;
    let says_owed = |a: &Answer| a.value.as_str() == Some(triage::OWED);
    if taught_no_ask(answers)
        || answers
            .iter()
            .filter(money)
            .any(|a| a.acted && !says_owed(a))
    {
        return None;
    }
    answers
        .iter()
        .filter(money)
        .filter(|a| says_owed(a))
        .find(|a| {
            a.acted
                || (a.basis == NeedBasis::Model
                    && a.confidence.is_some_and(|c| c >= settings.check_floor()))
        })
}

/// Whether a rule the member taught says this mail asks nothing of them: no
/// bill to pay either.
fn taught_no_ask(answers: &[Answer]) -> bool {
    answers
        .iter()
        .any(|a| a.basis == NeedBasis::Taught && a.question == triage::ASKS_ACTION && !a.is_true())
}

/// The need the answers about `message` raise, if any, under `settings`.
pub fn raise(message: MessageId, answers: &[Answer], settings: &NeedSettings) -> Option<Need> {
    let check_floor = settings.check_floor();
    let need = |reason, trust, basis, question, decision: &Answer| Need {
        message,
        reason,
        trust,
        basis,
        question,
        decision: Some(decision.id),
        due_on: None,
        urgent: false,
    };
    // A suspicious message is only ever a warning: its asks are not ours.
    let suspicious = answers
        .iter()
        .find(|a| a.question == triage::SUSPICIOUS && a.is_true() && a.acted);
    if let Some(answer) = suspicious {
        return Some(need(
            NeedReason::Review,
            NeedTrust::Sure,
            answer.basis,
            NeedQuestion::Suspicious,
            answer,
        ));
    }
    let answers = taught_first(answers)?;
    let answers = answers.as_slice();
    let stream = sorted_stream(answers, &settings.policy);
    // A rule the member taught ("needs a reply", "asks for an action")
    // speaks for them, whatever the stream.
    let taught_ask = answers.iter().any(|a| {
        a.basis == NeedBasis::Taught
            && a.is_true()
            && (a.question == triage::NEEDS_REPLY || a.question == triage::ASKS_ACTION)
    });
    let may_ask = taught_ask || may_ask(stream, answers);
    let asks = accepted(answers, triage::ASKS_ACTION);
    let owed = owed(answers, settings).filter(|_| stream != Some(NEWSLETTERS));
    let raised = if let Some(owed) = owed {
        // Campbooks cannot see the bank: a payment still owed is always a
        // question, "Is it paid?", until a member answers it.
        Some(need(
            NeedReason::Pay,
            NeedTrust::Check,
            owed.basis,
            NeedQuestion::Money,
            owed,
        ))
    } else if !may_ask || (stream == Some(RECEIPTS) && !taught_ask) {
        // Money mail asks for nothing but a payment, and the money answer
        // says none is owed (a receipt, a payment confirmation): nothing.
        None
    } else if let Some(reply) = accepted(answers, triage::NEEDS_REPLY) {
        Some(need(
            NeedReason::Reply,
            NeedTrust::Sure,
            reply.basis,
            NeedQuestion::NeedsReply,
            reply,
        ))
    } else if let Some(asks) = asks {
        Some(need(
            NeedReason::Do,
            NeedTrust::Sure,
            asks.basis,
            NeedQuestion::AsksAction,
            asks,
        ))
    } else {
        // A model suggestion the policy does not accept, put as a question,
        // unless an answer that acted already said no: the rules were sure
        // (a notification asks nothing of you), so there is nothing to ask.
        [
            (triage::NEEDS_REPLY, NeedQuestion::NeedsReply),
            (triage::ASKS_ACTION, NeedQuestion::AsksAction),
        ]
        .into_iter()
        .filter(|(id, _)| !settled_no(answers, id))
        .find_map(|(id, question)| {
            answers
                .iter()
                .find(|a| {
                    a.question == id
                        && a.basis == NeedBasis::Model
                        && !a.acted
                        && a.is_true()
                        && a.confidence.is_some_and(|c| c >= check_floor)
                })
                .map(|a| {
                    need(
                        NeedReason::Review,
                        NeedTrust::Check,
                        NeedBasis::Model,
                        question,
                        a,
                    )
                })
        })
    };
    let raised = raised.map(|mut need| {
        need.urgent = answers.iter().any(|a| {
            a.question == triage::URGENCY && a.acted && a.level().is_some_and(|l| l >= TODAY_LEVEL)
        });
        need
    });
    // Anything that asks something of the member comes first.
    raised.or_else(|| highlighted(message, answers))
}

/// "Worth a look": what a rule the member taught keeps in front of them
/// until they have opened it, when nothing asks more of them. It is never
/// urgent and never a question: they asked for it.
fn highlighted(message: MessageId, answers: &[Answer]) -> Option<Need> {
    answers
        .iter()
        .find(|a| a.basis == NeedBasis::Taught && a.question == WORTH_A_LOOK && a.is_true())
        .map(|a| Need {
            message,
            reason: NeedReason::WorthALook,
            trust: NeedTrust::Sure,
            basis: a.basis,
            question: NeedQuestion::WorthALook,
            decision: Some(a.id),
            due_on: None,
            urgent: false,
        })
}

/// The answers with a taught rule's speaking for the member: its answer to
/// a question replaces the signals' and the model's; `None` when it says
/// this mail never needs anyone.
fn taught_first(answers: &[Answer]) -> Option<Vec<Answer>> {
    if surfaces_never(answers) {
        return None;
    }
    let taught: Vec<&str> = answers
        .iter()
        .filter(|a| a.basis == NeedBasis::Taught)
        .map(|a| a.question.as_str())
        .collect();
    Some(
        answers
            .iter()
            .filter(|a| a.basis == NeedBasis::Taught || !taught.contains(&a.question.as_str()))
            .cloned()
            .collect(),
    )
}

/// Whether a taught rule says this mail never needs anyone.
fn surfaces_never(answers: &[Answer]) -> bool {
    answers.iter().any(|a| {
        a.basis == NeedBasis::Taught && a.question == SURFACE && a.value.as_str() == Some("never")
    })
}

/// Whether a taught rule now says the need raised by `question` is not
/// needed: "never", or a false answer to that question. A warning about
/// suspicious mail is never cleared this way.
fn rule_clears(answers: &[Answer], question: NeedQuestion) -> bool {
    if question == NeedQuestion::Suspicious {
        return false;
    }
    surfaces_never(answers)
        || (question == NeedQuestion::Money && taught_no_ask(answers))
        || answers.iter().any(|a| {
            a.basis == NeedBasis::Taught && a.question == question.as_str() && !a.is_true()
        })
}

/// What happened in the conversation since the need's message.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)]
pub struct Since {
    /// A message was sent from the inbox after it.
    pub replied: bool,
    /// The conversation is archived.
    pub archived: bool,
    /// Every message is in the trash or spam, or the message is gone or in
    /// spam or the trash itself.
    pub trashed: bool,
    /// A member's verdict on this message's need.
    pub verdict: Option<NeedVerdict>,
    /// What the inbox's taught rules say about it.
    pub rule: RuleSays,
    /// The message was opened (read) by someone, here or at the provider.
    pub opened: bool,
}

/// What a taught rule says about a need.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum RuleSays {
    /// Nothing.
    #[default]
    Nothing,
    /// It is not needed.
    NotNeeded,
}

/// A need, settled.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Settled {
    /// It needs the inbox's members.
    Open(Need),
    /// It no longer does, and why.
    Cleared(Need, ClearCause),
}

/// Settle `need` against what happened since (the clearing table of the
/// design).
pub fn settle(mut need: Need, since: Since) -> Settled {
    match since.verdict {
        // Paid already is done for a Pay need: nothing is filed yet.
        Some(NeedVerdict::Done | NeedVerdict::AlreadyPaid) => {
            return Settled::Cleared(need, ClearCause::Done);
        }
        Some(NeedVerdict::NotNeeded) => return Settled::Cleared(need, ClearCause::NotNeeded),
        Some(NeedVerdict::Confirmed | NeedVerdict::StillToPay) => {
            // "Yes, it needs a reply": the question becomes what it asked.
            if need.trust == NeedTrust::Check {
                need.reason = match need.question {
                    NeedQuestion::AsksAction => NeedReason::Do,
                    NeedQuestion::Money => NeedReason::Pay,
                    NeedQuestion::NeedsReply | NeedQuestion::Suspicious => NeedReason::Reply,
                    NeedQuestion::WorthALook => NeedReason::WorthALook,
                };
            }
            need.trust = NeedTrust::Sure;
            need.basis = NeedBasis::Member;
        }
        None => {}
    }
    if since.rule == RuleSays::NotNeeded
        && need.basis != NeedBasis::Member
        && need.question != NeedQuestion::Suspicious
    {
        return Settled::Cleared(need, ClearCause::Rule);
    }
    if since.trashed {
        return Settled::Cleared(need, ClearCause::Trashed);
    }
    // "Worth a look" asks only that the member opens it once.
    if since.opened && need.reason == NeedReason::WorthALook {
        return Settled::Cleared(need, ClearCause::Opened);
    }
    if since.replied && matches!(need.reason, NeedReason::Reply | NeedReason::Do) {
        return Settled::Cleared(need, ClearCause::Replied);
    }
    // Archiving says you read it, not that you paid or did it (owner,
    // 2026-10-13): Do and Pay stay until Done or Not needed.
    if since.archived && matches!(need.reason, NeedReason::Reply | NeedReason::Review) {
        return Settled::Cleared(need, ClearCause::Archived);
    }
    Settled::Open(need)
}

/// The stored row.
#[derive(Debug)]
struct Row {
    message_id: MessageId,
    reason: String,
    trust: String,
    basis: String,
    question: String,
    decision_id: Option<DecisionId>,
    due_on: Option<NaiveDate>,
    urgent: bool,
    state: String,
    cleared_because: Option<String>,
    verdict: Option<String>,
    verdict_message_id: Option<MessageId>,
    verdict_by: Option<UserId>,
    latest_inbound_id: Option<MessageId>,
}

impl Row {
    fn need(&self) -> Result<Need> {
        Ok(Need {
            message: self.message_id,
            reason: self.reason.parse()?,
            trust: self.trust.parse()?,
            basis: self.basis.parse()?,
            question: self.question.parse()?,
            decision: self.decision_id,
            due_on: self.due_on,
            urgent: self.urgent,
        })
    }

    fn verdict_on(&self, message: MessageId) -> Result<Option<NeedVerdict>> {
        match (&self.verdict, self.verdict_message_id) {
            (Some(verdict), Some(on)) if on == message => Ok(Some(verdict.parse()?)),
            _ => Ok(None),
        }
    }
}

async fn current(conn: &mut PgConnection, conversation: ConversationId) -> Result<Option<Row>> {
    Ok(sqlx::query_as!(
        Row,
        r#"
        SELECT message_id AS "message_id: MessageId", reason, trust, basis, question,
               decision_id AS "decision_id: DecisionId", due_on, urgent, state, cleared_because,
               verdict, verdict_message_id AS "verdict_message_id: MessageId",
               verdict_by AS "verdict_by: UserId",
               latest_inbound_id AS "latest_inbound_id: MessageId"
          FROM conversations.needs WHERE conversation_id = $1 FOR UPDATE
        "#,
        conversation as ConversationId,
    )
    .fetch_optional(conn)
    .await?)
}

/// The answers stored about `message`. A model's answer counts only when it
/// was given while the model passed its answers check
/// (`campbooks_ai::decide::check::STAMP`): the rest are kept, never read.
async fn answers(conn: &mut PgConnection, message: MessageId) -> Result<Vec<Answer>> {
    let rows = sqlx::query!(
        r#"
        SELECT id AS "id: DecisionId", question, source, value, confidence, acted, reasons
          FROM conversations.decisions
         WHERE message_id = $1 AND (source <> 'model' OR model_version LIKE 'checked:%')
        "#,
        message as MessageId,
    )
    .fetch_all(conn)
    .await?;
    rows.into_iter()
        .map(|row| {
            Ok(Answer {
                id: row.id,
                question: row.question,
                basis: match row.source.as_str() {
                    "rules" => NeedBasis::Rules,
                    "taught" => NeedBasis::Taught,
                    _ => NeedBasis::Model,
                },
                value: row.value,
                confidence: row.confidence,
                acted: row.acted,
                reasons: serde_json::from_value(row.reasons)
                    .map_err(|e| Error::internal(super::redact::json(&e)))?,
            })
        })
        .collect()
}

/// The need a conversation has now, before settling.
struct Candidate {
    /// What its newest inbound message raises when that is recent (or is
    /// the one the stored need already comes from), else the stored need
    /// while it is open.
    need: Option<Need>,
    /// Its newest inbound message.
    latest: Option<MessageId>,
    /// The stored open need's own message, read again, raises nothing now
    /// (its answers no longer support it, e.g. a question the rules had
    /// already settled), and no member confirmed it: it is withdrawn.
    withdrawn: bool,
}

/// The need conversation `id` has now, before settling.
async fn candidate(
    tx: &mut Transaction<'_, Postgres>,
    id: ConversationId,
    row: Option<&Row>,
    settings: &NeedSettings,
) -> Result<Candidate> {
    let newest = sqlx::query!(
        r#"SELECT id AS "id: MessageId", sent_at FROM conversations.messages
            WHERE conversation_id = $1 AND direction = 'inbound'
              AND NOT in_spam AND NOT in_trash
            ORDER BY sent_at DESC, id DESC LIMIT 1"#,
        id as ConversationId,
    )
    .fetch_optional(&mut **tx)
    .await?;

    // The newest inbound message raises the need when it is recent (or is
    // the one the stored need already comes from); else an open need stays
    // as it is, settled against what happened since.
    let mut raised = None;
    let mut reread = false;
    let mut recent = false;
    let mut money_asked = true;
    if let Some(newest) = &newest {
        recent = newest.sent_at >= Utc::now() - settings.window;
        reread = row.is_some_and(|r| r.message_id == newest.id);
        if recent || reread {
            let answers = answers(tx, newest.id).await?;
            money_asked = answers.iter().any(|a| a.question == triage::MONEY);
            raised = raise(newest.id, &answers, settings);
        }
    }
    let open = row.filter(|r| r.state == "open");
    let withdrawn = match open {
        // A bill raised before the money question, on mail recent enough to
        // be asked it, stays until it is answered: a pass that could not ask
        // (the daily AI cap, a model that failed the answers check) must not
        // drop a bill that may still be owed.
        Some(row)
            if reread
                && raised.is_none()
                && recent
                && !money_asked
                && row.need()?.reason == NeedReason::Pay =>
        {
            false
        }
        Some(row) if reread && raised.is_none() => !matches!(
            row.verdict_on(row.message_id)?,
            Some(NeedVerdict::Confirmed | NeedVerdict::StillToPay)
        ),
        _ => false,
    };
    let need = match (raised, open) {
        (Some(need), _) => Some(need),
        (None, Some(row)) => Some(row.need()?),
        (None, None) => None,
    };
    Ok(Candidate {
        need,
        latest: newest.map(|n| n.id),
        withdrawn,
    })
}

/// What happened in conversation `id` since `need`'s message, given whether
/// the conversation is archived and trashed (`state`); and when that message
/// was sent (`None` once it is gone).
async fn since(
    tx: &mut Transaction<'_, Postgres>,
    id: ConversationId,
    state: &(bool, bool),
    row: Option<&Row>,
    need: &Need,
) -> Result<(Since, Option<DateTime<Utc>>)> {
    // A message in spam or the trash needs nothing, as one that is gone.
    let sent = sqlx::query_scalar!(
        "SELECT sent_at FROM conversations.messages
          WHERE id = $1 AND NOT in_spam AND NOT in_trash",
        need.message as MessageId,
    )
    .fetch_optional(&mut **tx)
    .await?;
    let replied = match sent {
        Some(sent) => {
            sqlx::query_scalar!(
                r#"SELECT EXISTS (SELECT 1 FROM conversations.messages
                 WHERE conversation_id = $1 AND direction = 'outbound' AND sent_at > $2) AS "e!""#,
                id as ConversationId,
                sent,
            )
            .fetch_one(&mut **tx)
            .await?
        }
        None => false,
    };
    let verdict = match row {
        Some(row) => row.verdict_on(need.message)?,
        None => None,
    };
    let opened: bool = sqlx::query_scalar("SELECT read FROM conversations.messages WHERE id = $1")
        .bind(need.message)
        .fetch_optional(&mut **tx)
        .await?
        .unwrap_or(false);
    let rule = if rule_clears(&answers(tx, need.message).await?, need.question) {
        RuleSays::NotNeeded
    } else {
        RuleSays::Nothing
    };
    Ok((
        Since {
            replied,
            archived: state.0,
            trashed: state.1 || sent.is_none(),
            verdict,
            rule,
            opened,
        },
        sent,
    ))
}

/// A conversation whose need can change.
struct LiveConversation {
    workspace_id: sqlx::types::Uuid,
    inbox: InboxId,
    archived: bool,
    trashed: bool,
}

/// Conversation `id`, unless it is gone or in a kept, read-only inbox (its
/// connection removed): that is history, and nothing in it changes what it
/// needs.
async fn live_conversation(
    tx: &mut Transaction<'_, Postgres>,
    id: ConversationId,
) -> Result<Option<LiveConversation>> {
    let row = sqlx::query!(
        r#"SELECT c.workspace_id, c.inbox_id AS "inbox: InboxId",
                  c.archived_at IS NOT NULL AS "archived!", c.trashed,
                  i.detached_at IS NOT NULL AS "detached!"
             FROM conversations.conversations c
             JOIN conversations.inboxes i ON i.id = c.inbox_id
            WHERE c.id = $1"#,
        id as ConversationId,
    )
    .fetch_optional(&mut **tx)
    .await?;
    Ok(row.filter(|r| !r.detached).map(|r| LiveConversation {
        workspace_id: r.workspace_id,
        inbox: r.inbox,
        archived: r.archived,
        trashed: r.trashed,
    }))
}

/// What the check of a first need's source message found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SourceCheck {
    /// It can carry the need.
    Carries,
    /// It is in spam or the trash (or gone): no need, now or later.
    Cannot,
    /// Another transaction is changing it right now: undecided.
    Busy,
}

/// How a refresh ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refreshed {
    /// Settled (stored, cleared, or nothing to change).
    Settled,
    /// A first need was not stored because its message is being changed by
    /// another transaction; the need is still to be worked out.
    Deferred,
}

/// How long the follow-up job waits for a lock before it gives up and
/// retries.
const JOB_LOCK_TIMEOUT: &str = "5s";

/// Check the conversation and the message an open first need is about to be
/// stored for.
///
/// The race: a decide pass reads a message before a provider move to spam or
/// the trash commits, then stores an open need after it. It exists only when
/// the conversation has no needs row yet: with one, `current` locks it `FOR
/// UPDATE`, which serialises refreshes, and the move's own refresh (the end
/// of `apply_changes`) clears a stale need. So this runs only for a first
/// insert.
///
/// What it takes, in this order, and keeps until the transaction ends:
/// 1. the conversation row `FOR KEY SHARE`, which is what the foreign key of
///    the needs insert would take anyway. Taking it first means the insert
///    can no longer wait for a conversation lock a writer holds (flags and
///    rules lock the conversation `FOR UPDATE` and then update messages,
///    which would wait on the share lock below: a cycle if we took the
///    message first). A lock held by another transaction is not waited for
///    (`SKIP LOCKED`); the same transaction's own lock is not skipped, so
///    `judge`, `reopen`, flags and rules refreshing their own conversation
///    are unaffected (and the first two always have a needs row anyway).
/// 2. the message row `FOR SHARE`, re-read under the lock. Every later
///    writer of that message (a move, a flag, a template fill) waits for
///    this transaction to commit.
///
/// In the request and event paths nothing waits: a row someone else holds is
/// [`SourceCheck::Busy`], nothing is stored, and the refresh is
/// [`Refreshed::Deferred`], which [`refresh`] turns into a durable
/// `conversations.refresh_need` job. That is safe whatever the writer is: not
/// every writer refreshes the conversation (`fill_templates` does not), so
/// the follow-up does not depend on it.
///
/// The follow-up job (`wait`) holds no needs row and no message lock when it
/// gets here (plain reads so far), so it may wait: both locks are taken
/// blocking under a 5 s `lock_timeout`, and a timeout is
/// [`SourceCheck::Busy`] (the job retries). While it waits for the message it
/// holds the conversation `FOR KEY SHARE`, which a request taking that
/// conversation `FOR UPDATE` waits on; a longer cycle (with a rule run over
/// several conversations) is possible, and bounded: the deadlock detector or
/// the lock timeout breaks it and the job retries. Because its reads predate
/// the wait, once it holds both locks the refresh is worked out again, once,
/// in the same transaction and without waiting (see [`refresh_now`]): a
/// newer message that someone else holds makes it retry; waiting means a
/// long-held lock does not burn its attempts.
///
/// Lock order of every caller of [`refresh`] (the order before it reaches
/// this check): `apply_changes`, messages, then the conversation's counts,
/// then here; flags (`flags::apply`), conversation, then messages, then here;
/// rules (`rules/apply`), the conversation, then messages, then here; `judge`, `reopen`,
/// conversation, then the needs row (never here); the decide pass, the
/// backfill and the follow-up job, the needs row (none yet) then here. This
/// check never waits except in the job, so it adds no wait edge to any of
/// them.
async fn source_can_carry(
    tx: &mut Transaction<'_, Postgres>,
    conversation: ConversationId,
    message: MessageId,
    wait: bool,
) -> Result<SourceCheck> {
    let (conversation_sql, message_sql) = if wait {
        (
            "SELECT 1 FROM conversations.conversations WHERE id = $1 FOR KEY SHARE",
            "SELECT NOT in_spam AND NOT in_trash FROM conversations.messages
              WHERE id = $1 FOR SHARE",
        )
    } else {
        (
            "SELECT 1 FROM conversations.conversations WHERE id = $1
               FOR KEY SHARE SKIP LOCKED",
            "SELECT NOT in_spam AND NOT in_trash FROM conversations.messages
              WHERE id = $1 FOR SHARE SKIP LOCKED",
        )
    };
    // A savepoint: a lock timeout aborts only this part of the transaction.
    let mut sp = sqlx::Connection::begin(&mut **tx).await?;
    // The timeout in force, put back afterwards (transaction-local, not a
    // session-wide RESET).
    let mut previous_timeout: Option<String> = None;
    if wait {
        previous_timeout = Some(
            sqlx::query_scalar("SELECT current_setting('lock_timeout')")
                .fetch_one(&mut *sp)
                .await?,
        );
        sqlx::query("SELECT set_config('lock_timeout', $1, true)")
            .bind(JOB_LOCK_TIMEOUT)
            .execute(&mut *sp)
            .await?;
    }
    let locked = async {
        let conversation_held: Option<i32> = sqlx::query_scalar(conversation_sql)
            .bind(conversation)
            .fetch_optional(&mut *sp)
            .await?;
        if conversation_held.is_none() {
            return Ok::<_, sqlx::Error>(None);
        }
        let free: Option<bool> = sqlx::query_scalar(message_sql)
            .bind(message)
            .fetch_optional(&mut *sp)
            .await?;
        Ok(Some(free))
    }
    .await;
    match &locked {
        Err(e)
            if e.as_database_error()
                .and_then(sqlx::error::DatabaseError::code)
                .as_deref()
                == Some("55P03") =>
        {
            sp.rollback().await?;
            return Ok(SourceCheck::Busy);
        }
        Err(_) => {
            locked.map_err(Error::from)?;
            unreachable!("the error was returned above");
        }
        Ok(_) => {}
    }
    if let Some(previous) = previous_timeout {
        sqlx::query("SELECT set_config('lock_timeout', $1, true)")
            .bind(previous)
            .execute(&mut *sp)
            .await?;
    }
    sp.commit().await?;
    match locked.map_err(Error::from)? {
        // The conversation row is held by another transaction.
        None => Ok(SourceCheck::Busy),
        Some(Some(true)) => Ok(SourceCheck::Carries),
        Some(Some(false)) => Ok(SourceCheck::Cannot),
        // `SKIP LOCKED` says nothing for a locked row and for a missing one;
        // a plain read tells them apart and never waits.
        Some(None) => {
            let exists: bool = sqlx::query_scalar(
                "SELECT EXISTS (SELECT 1 FROM conversations.messages WHERE id = $1)",
            )
            .bind(message)
            .fetch_one(&mut **tx)
            .await?;
            Ok(if exists {
                SourceCheck::Busy
            } else {
                SourceCheck::Cannot
            })
        }
    }
}

/// Work out conversation `id`'s need again and, when it changed, store it
/// and publish `conversations.need_changed` in `tx`. A conversation that is
/// gone, has no inbound message, or is in a kept read-only inbox
/// (`detached_at`) changes nothing.
///
/// A first need whose message another transaction is changing is not stored
/// now; a `conversations.refresh_need` job, enqueued in `tx`, works it out
/// again a few seconds later.
pub async fn refresh(
    tx: &mut Transaction<'_, Postgres>,
    events: &EventBus,
    workspace: &str,
    id: ConversationId,
    settings: &NeedSettings,
) -> Result<()> {
    if refresh_now(tx, events, workspace, id, settings, false).await? == Refreshed::Deferred {
        crate::features::refresh_need::enqueue(tx, workspace, id).await?;
    }
    Ok(())
}

/// [`refresh`] without the follow-up: says whether it deferred. `wait` is
/// for the follow-up job only (see [`source_can_carry`]).
pub async fn refresh_now(
    tx: &mut Transaction<'_, Postgres>,
    events: &EventBus,
    workspace: &str,
    id: ConversationId,
    settings: &NeedSettings,
    wait: bool,
) -> Result<Refreshed> {
    match refresh_pass(tx, events, workspace, id, settings, wait, false).await? {
        Pass::Done(done) => Ok(done),
        // Once only, and without waiting: the second pass reads everything
        // again and may pick a newer message than the one the job holds; its
        // own locks pass at once, and a message someone else holds is Busy
        // (the job rolls back and retries with fresh reads), so it never
        // waits on reads it made before a wait.
        Pass::Again => {
            match refresh_pass(tx, events, workspace, id, settings, false, true).await? {
                Pass::Done(done) => Ok(done),
                Pass::Again => unreachable!("the second pass does not ask again"),
            }
        }
    }
}

/// How one pass of [`refresh_now`] ended.
enum Pass {
    Done(Refreshed),
    /// The waiting job got its locks after reading: read everything again.
    Again,
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
async fn refresh_pass(
    tx: &mut Transaction<'_, Postgres>,
    events: &EventBus,
    workspace: &str,
    id: ConversationId,
    settings: &NeedSettings,
    wait: bool,
    again: bool,
) -> Result<Pass> {
    let Some(conversation) = live_conversation(tx, id).await? else {
        return Ok(Pass::Done(Refreshed::Settled));
    };
    let conversation_state = (conversation.archived, conversation.trashed);
    let row = current(tx, id).await?;
    let Candidate {
        need,
        latest,
        withdrawn,
    } = candidate(tx, id, row.as_ref(), settings).await?;
    let Some(need) = need else {
        return Ok(Pass::Done(Refreshed::Settled));
    };

    let (since, sent) = since(tx, id, &conversation_state, row.as_ref(), &need).await?;
    let settled = match settle(need, since) {
        // Its own message no longer raises it: cleared as replaced.
        Settled::Open(need) if withdrawn => Settled::Cleared(need, ClearCause::Superseded),
        settled => settled,
    };

    let (need, cause) = match &settled {
        Settled::Open(need) => (need, None),
        Settled::Cleared(need, cause) => (need, Some(*cause)),
    };
    // A first need for a message a move may be changing right now: store
    // nothing, the move's refresh decides (see `source_can_carry`).
    if row.is_none() && cause.is_none() {
        match source_can_carry(tx, id, need.message, wait).await? {
            // The job waited for its locks: everything read so far may
            // predate what it waited for (an archive, another refresh's
            // need), so work it out again, once, with the locks held.
            SourceCheck::Carries if wait && !again => return Ok(Pass::Again),
            SourceCheck::Carries => {}
            SourceCheck::Cannot => return Ok(Pass::Done(Refreshed::Settled)),
            SourceCheck::Busy => return Ok(Pass::Done(Refreshed::Deferred)),
        }
    }
    if !worth_announcing(row.as_ref(), need, cause, latest)? {
        return Ok(Pass::Done(Refreshed::Settled));
    }
    let cleared_by = match cause {
        Some(ClearCause::Done | ClearCause::NotNeeded) => row.as_ref().and_then(|r| r.verdict_by),
        _ => None,
    };
    let revision = sqlx::query_scalar!(
        r#"
        INSERT INTO conversations.needs
            (conversation_id, workspace_id, message_id, reason, trust, basis, question,
             decision_id, due_on, urgent, state, cleared_because, cleared_by, cleared_at,
             latest_inbound_id)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13,
                CASE WHEN $12::text IS NULL THEN NULL ELSE now() END, $14)
        ON CONFLICT (conversation_id) DO UPDATE
           SET message_id = EXCLUDED.message_id, reason = EXCLUDED.reason,
               trust = EXCLUDED.trust, basis = EXCLUDED.basis, question = EXCLUDED.question,
               decision_id = EXCLUDED.decision_id, due_on = EXCLUDED.due_on,
               urgent = EXCLUDED.urgent, state = EXCLUDED.state,
               cleared_because = EXCLUDED.cleared_because, cleared_by = EXCLUDED.cleared_by,
               cleared_at = EXCLUDED.cleared_at, latest_inbound_id = EXCLUDED.latest_inbound_id,
               revision = conversations.needs.revision + 1, updated_at = now()
        RETURNING revision
        "#,
        id as ConversationId,
        conversation.workspace_id,
        need.message as MessageId,
        need.reason.as_str(),
        need.trust.as_str(),
        need.basis.as_str(),
        need.question.as_str(),
        need.decision as Option<DecisionId>,
        need.due_on,
        need.urgent,
        if cause.is_some() { "cleared" } else { "open" },
        cause.map(ClearCause::as_str),
        cleared_by.map(|u| *u.as_uuid()),
        latest as Option<MessageId>,
    )
    .fetch_one(&mut **tx)
    .await?;
    // A need that opens for a message (new, or back after being cleared)
    // gets Scout's summary of the email, through a job: never here, and
    // once per message however often the need is worked out again. Mail
    // that may be phishing gets none: a summary would retell the scam in
    // Scout's voice, and the warning is what matters there.
    let opened = cause.is_none()
        && row
            .as_ref()
            .is_none_or(|r| r.state != "open" || r.message_id != need.message);
    if settings.summaries && opened && need.question != NeedQuestion::Suspicious {
        crate::features::summarize_need::enqueue(tx, workspace, need.message).await?;
    }
    events
        .publish(
            tx,
            &NeedChanged {
                workspace: workspace.to_owned(),
                inbox: conversation.inbox,
                conversation: id,
                message: need.message,
                revision,
                sent_at: sent.unwrap_or_else(Utc::now),
                need: match cause {
                    None => Some(NeedFacts {
                        reason: need.reason,
                        trust: need.trust,
                        due_on: need.due_on,
                        urgent: need.urgent,
                    }),
                    Some(_) => None,
                },
                cleared_because: cause,
                cleared_by,
                latest_message: latest,
            },
        )
        .await?;
    Ok(Pass::Done(Refreshed::Settled))
}

/// Whether settling `need` (cleared by `cause`, if any) changes what was
/// announced in `row`, given the conversation's newest inbound message.
fn worth_announcing(
    row: Option<&Row>,
    need: &Need,
    cause: Option<ClearCause>,
    latest: Option<MessageId>,
) -> Result<bool> {
    let Some(row) = row else {
        // A need that was never open is not announced as cleared.
        return Ok(cause.is_none());
    };
    if cause.is_some() && row.state != "open" && row.message_id != need.message {
        return Ok(false);
    }
    // The sender writing again is news even when it raises nothing (it ends
    // a member's Later), as long as the need is still open.
    let newer_message = row.state == "open" && cause.is_none() && row.latest_inbound_id != latest;
    Ok(newer_message || !unchanged(row, need, cause)?)
}

fn unchanged(row: &Row, need: &Need, cause: Option<ClearCause>) -> Result<bool> {
    let stored_cause = row
        .cleared_because
        .as_deref()
        .map(str::parse::<ClearCause>)
        .transpose()?;
    Ok(row.need()? == *need && stored_cause == cause)
}

/// Announce that conversation `id`'s open need is gone with the
/// conversation (removed by the provider, or merged into another), before
/// the row goes with it.
pub async fn conversation_gone(
    tx: &mut Transaction<'_, Postgres>,
    events: &EventBus,
    workspace: &str,
    inbox: InboxId,
    id: ConversationId,
) -> Result<()> {
    let row = sqlx::query!(
        r#"
        UPDATE conversations.needs
           SET state = 'cleared', cleared_because = 'superseded', cleared_at = now(),
               revision = revision + 1, updated_at = now()
         WHERE conversation_id = $1 AND state = 'open'
        RETURNING message_id AS "message: MessageId", revision
        "#,
        id as ConversationId,
    )
    .fetch_optional(&mut **tx)
    .await?;
    if let Some(row) = row {
        events
            .publish(
                tx,
                &NeedChanged {
                    workspace: workspace.to_owned(),
                    inbox,
                    conversation: id,
                    message: row.message,
                    revision: row.revision,
                    sent_at: Utc::now(),
                    need: None,
                    cleared_because: Some(ClearCause::Superseded),
                    cleared_by: None,
                    latest_message: None,
                },
            )
            .await?;
    }
    Ok(())
}

/// The sender's sentence a need quotes, with who wrote it and when.
/// Untrusted mail content: clients show it as quoted plain text only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct Quote {
    /// The sentence, at most 240 characters.
    pub text: String,
    /// Who wrote it.
    pub from: Address,
    /// When.
    pub sent_at: DateTime<Utc>,
}

/// A conversation's open need, as lists and the reader show it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct NeedView {
    /// Why.
    pub reason: NeedReason,
    /// `sure`, or `check` for a question to the member.
    pub trust: NeedTrust,
    /// Whose answer raised it.
    pub basis: NeedBasis,
    /// The question that raised it.
    pub question: NeedQuestion,
    /// The rule that decided it (`model.decide`, …), for the sentence the
    /// client words it with.
    #[schema(example = "model.decide")]
    pub rule: Option<String>,
    /// When it is due: none until a due date is extracted from the mail
    /// (needs raised before ADR 0018 may carry one).
    pub due_on: Option<NaiveDate>,
    /// Whether the due date has passed.
    pub overdue: bool,
    /// Whether it is for today.
    pub urgent: bool,
    /// The message it comes from.
    pub message_id: MessageId,
    /// What the sender wrote, when an answer stored before ADR 0018 quoted
    /// it.
    pub quote: Option<Quote>,
    /// Its own actions, rated by the `decide` model from the action
    /// catalogue, at most three, the first to offer first
    /// (campbooks-cloud#166). Absent unless the catalogue is switched on
    /// (`CAMPBOOKS_CONVERSATIONS__ACTIONS__ENABLED`) and the message was
    /// rated: the client then offers its fixed actions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actions: Option<Vec<super::actions::SuggestedAction>>,
}

/// Fill in each summary's open [`NeedView`].
pub async fn attach(conn: &mut PgConnection, summaries: &mut [ConversationSummary]) -> Result<()> {
    let ids: Vec<ConversationId> = summaries.iter().map(|s| s.id).collect();
    if ids.is_empty() {
        return Ok(());
    }
    let rows = sqlx::query!(
        r#"
        SELECT n.conversation_id AS "conversation_id: ConversationId",
               n.message_id AS "message_id: MessageId", n.reason, n.trust, n.basis, n.question,
               n.due_on, n.urgent, m.sender AS "sender?", m.sent_at AS "sent_at?",
               d.reasons AS "reasons?"
          FROM conversations.needs n
          LEFT JOIN conversations.messages m ON m.id = n.message_id
          LEFT JOIN conversations.decisions d ON d.id = n.decision_id
         WHERE n.conversation_id = ANY($1) AND n.state = 'open'
        "#,
        &ids as &[ConversationId],
    )
    .fetch_all(&mut *conn)
    .await?;
    let today = Utc::now().date_naive();
    for row in rows {
        let Some(summary) = summaries.iter_mut().find(|s| s.id == row.conversation_id) else {
            continue;
        };
        let reasons: Vec<StoredReason> = row
            .reasons
            .map(serde_json::from_value)
            .transpose()
            .map_err(|e| Error::internal(super::redact::json(&e)))?
            .unwrap_or_default();
        let from: Option<Address> = row
            .sender
            .map(serde_json::from_value)
            .transpose()
            .map_err(|e| Error::internal(super::redact::json(&e)))?;
        let quote = match (
            reasons.iter().find_map(|r| r.sentence.clone()),
            from,
            row.sent_at,
        ) {
            (Some(text), Some(from), Some(sent_at)) => Some(Quote {
                text,
                from,
                sent_at,
            }),
            _ => None,
        };
        summary.need = Some(NeedView {
            reason: row.reason.parse()?,
            trust: row.trust.parse()?,
            basis: row.basis.parse()?,
            question: row.question.parse()?,
            rule: reasons.first().map(|r| r.rule.clone()),
            due_on: row.due_on,
            overdue: row.due_on.is_some_and(|due| due < today),
            urgent: row.urgent,
            message_id: row.message_id,
            quote,
            actions: None,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn answer(question: &str, basis: NeedBasis, value: serde_json::Value, acted: bool) -> Answer {
        Answer {
            id: DecisionId::new(),
            question: question.into(),
            basis,
            value,
            confidence: Some(0.9),
            acted,
            reasons: Vec::new(),
        }
    }

    fn with_reason(mut a: Answer, rule: &str, evidence: &str) -> Answer {
        a.reasons.push(StoredReason {
            rule: rule.into(),
            evidence: evidence.into(),
            sentence: None,
        });
        a
    }

    fn stream(basis: NeedBasis, label: &str, acted: bool) -> Answer {
        answer(triage::STREAM, basis, json!(label), acted)
    }

    /// The shipped policy, with every model answer acting from 0.85.
    fn settings() -> NeedSettings {
        let mut policy = Policy::baseline();
        for question in [triage::STREAM, triage::NEEDS_REPLY, triage::ASKS_ACTION] {
            policy = policy.with(
                question,
                Source::Model,
                super::super::policy::Switch::ActAbove(0.85),
            );
        }
        NeedSettings::new(14, &policy)
    }

    fn raised(answers: &[Answer]) -> Option<Need> {
        raise(MessageId::new(), answers, &settings())
    }

    #[test]
    fn a_taught_rule_answers_before_the_signals() {
        // "Needs a reply: no" from a rule beats the signals' yes.
        assert_eq!(
            raised(&[
                answer(triage::NEEDS_REPLY, NeedBasis::Rules, json!(true), true),
                answer(triage::NEEDS_REPLY, NeedBasis::Taught, json!(false), true),
            ]),
            None
        );
        // "Asks for an action: yes" from a rule raises Do on its own.
        let need = raised(&[answer(
            triage::ASKS_ACTION,
            NeedBasis::Taught,
            json!(true),
            true,
        )])
        .unwrap();
        assert_eq!(
            (need.reason, need.basis),
            (NeedReason::Do, NeedBasis::Taught)
        );
    }

    #[test]
    fn never_raises_nothing_but_a_warning_still_shows() {
        let never = answer(SURFACE, NeedBasis::Taught, json!("never"), true);
        assert_eq!(
            raised(&[
                answer(triage::ASKS_ACTION, NeedBasis::Rules, json!(true), true),
                never.clone(),
            ]),
            None
        );
        let warning = raised(&[
            answer(triage::SUSPICIOUS, NeedBasis::Model, json!(true), true),
            never,
        ])
        .unwrap();
        assert_eq!(warning.reason, NeedReason::Review);
    }

    #[test]
    fn a_highlight_rule_raises_worth_a_look_only_when_nothing_asks_more() {
        let look = answer(WORTH_A_LOOK, NeedBasis::Taught, json!(true), true);
        let need = raised(std::slice::from_ref(&look)).unwrap();
        assert_eq!(
            (need.reason, need.trust, need.basis, need.question),
            (
                NeedReason::WorthALook,
                NeedTrust::Sure,
                NeedBasis::Taught,
                NeedQuestion::WorthALook
            )
        );
        assert!(!need.urgent);
        // A reply someone is waiting for outranks it: one need per conversation.
        let reply = raised(&[
            answer(triage::NEEDS_REPLY, NeedBasis::Taught, json!(true), true),
            look.clone(),
        ])
        .unwrap();
        assert_eq!(reply.reason, NeedReason::Reply);
        // "Never needs anyone" is the opposite of a highlight.
        let never = answer(SURFACE, NeedBasis::Taught, json!("never"), true);
        assert_eq!(raised(&[look, never]), None);
    }

    #[test]
    fn worth_a_look_clears_itself_once_opened() {
        let opened = Since {
            opened: true,
            ..Since::default()
        };
        assert_eq!(
            cause(&settle(need(NeedReason::WorthALook), opened)),
            Some(ClearCause::Opened)
        );
        // Opening settles nothing else: a bill stays until answered.
        assert_eq!(cause(&settle(need(NeedReason::Pay), opened)), None);
        assert_eq!(
            cause(&settle(need(NeedReason::WorthALook), Since::default())),
            None
        );
        // Seen (Done) clears it too.
        let seen = Since {
            verdict: Some(NeedVerdict::Done),
            ..Since::default()
        };
        assert_eq!(
            cause(&settle(need(NeedReason::WorthALook), seen)),
            Some(ClearCause::Done)
        );
    }

    #[test]
    fn a_rule_clears_what_it_says_is_not_needed() {
        let by_rule = Since {
            rule: RuleSays::NotNeeded,
            ..Since::default()
        };
        assert_eq!(
            cause(&settle(need(NeedReason::Pay), by_rule)),
            Some(ClearCause::Rule)
        );
        let mut confirmed = need(NeedReason::Reply);
        confirmed.basis = NeedBasis::Member;
        assert_eq!(cause(&settle(confirmed, by_rule)), None);
        assert!(rule_clears(
            &[answer(
                triage::NEEDS_REPLY,
                NeedBasis::Taught,
                json!(false),
                true
            )],
            NeedQuestion::NeedsReply
        ));
        assert!(!rule_clears(
            &[answer(SURFACE, NeedBasis::Taught, json!("never"), true)],
            NeedQuestion::Suspicious
        ));
    }

    #[test]
    fn a_question_from_a_person_is_a_reply_with_no_due_date() {
        let need = raised(&[
            stream(NeedBasis::Model, "primary", true),
            answer(triage::NEEDS_REPLY, NeedBasis::Model, json!(true), true),
            answer(triage::ASKS_ACTION, NeedBasis::Model, json!(true), true),
            answer(triage::HAS_DEADLINE, NeedBasis::Model, json!(true), true),
            answer(triage::URGENCY, NeedBasis::Model, json!(1.0), false),
        ])
        .unwrap();
        assert_eq!(need.reason, NeedReason::Reply);
        assert_eq!(need.trust, NeedTrust::Sure);
        assert_eq!(need.basis, NeedBasis::Model);
        assert_eq!(need.due_on, None, "no due date until one is extracted");
        assert!(!need.urgent);
    }

    fn money(label: &str, confidence: f64, acted: bool) -> Answer {
        confident(
            answer(triage::MONEY, NeedBasis::Model, json!(label), acted),
            confidence,
        )
    }

    #[test]
    fn a_bill_is_a_pay_question() {
        let need = raised(&[
            stream(NeedBasis::Model, "receipts", true),
            answer(triage::ASKS_ACTION, NeedBasis::Model, json!(true), true),
            answer(triage::NEEDS_REPLY, NeedBasis::Model, json!(true), true),
            answer(triage::URGENCY, NeedBasis::Model, json!(2.0), true),
            money("owed", 0.91, false),
        ])
        .unwrap();
        assert_eq!(
            (need.reason, need.trust, need.question, need.basis),
            (
                NeedReason::Pay,
                NeedTrust::Check,
                NeedQuestion::Money,
                NeedBasis::Model
            )
        );
        assert!(need.urgent);
        assert_eq!(need.due_on, None, "no due date until one is read (#146)");
    }

    /// The bug of campbooks-cloud#167: an invoice already paid or a payment
    /// confirmation the model reads as asking something became "Pay".
    #[test]
    fn a_receipt_or_a_payment_confirmation_needs_nothing() {
        for label in ["paid", "neither"] {
            let receipt = [
                stream(NeedBasis::Model, "receipts", true),
                answer(triage::ASKS_ACTION, NeedBasis::Model, json!(true), true),
                answer(triage::HAS_DEADLINE, NeedBasis::Model, json!(true), true),
                money(label, 0.95, false),
            ];
            assert_eq!(raised(&receipt), None, "{label}");
        }
        // Answered before the money question was asked: nothing until the
        // model is asked again.
        let unasked = [
            stream(NeedBasis::Model, "receipts", true),
            answer(triage::ASKS_ACTION, NeedBasis::Model, json!(true), true),
        ];
        assert_eq!(raised(&unasked), None);
    }

    #[test]
    fn an_unsure_owed_is_no_pay() {
        let unsure = [
            stream(NeedBasis::Model, "receipts", true),
            answer(triage::ASKS_ACTION, NeedBasis::Model, json!(true), true),
            money("owed", 0.6, false),
        ];
        assert_eq!(raised(&unsure), None);
        // An answer the policy let act counts at any confidence, and an
        // acted answer saying otherwise wins over a confident "owed".
        let acted = [money("owed", 0.6, true)];
        assert_eq!(raised(&acted).unwrap().reason, NeedReason::Pay);
        let settled = [money("paid", 0.6, true), money("owed", 0.95, false)];
        assert_eq!(raised(&settled), None);
    }

    #[test]
    fn a_taught_no_ask_is_no_bill() {
        let answers = [
            stream(NeedBasis::Model, "receipts", true),
            money("owed", 0.95, false),
            answer(triage::ASKS_ACTION, NeedBasis::Taught, json!(false), true),
        ];
        assert_eq!(raised(&answers), None);
        assert!(rule_clears(&answers, NeedQuestion::Money));
    }

    #[test]
    fn an_invoice_sent_by_hand_is_pay_and_a_newsletter_never() {
        let by_hand = raised(&[
            stream(NeedBasis::Model, "primary", true),
            answer(triage::ASKS_ACTION, NeedBasis::Model, json!(true), true),
            answer(triage::NEEDS_REPLY, NeedBasis::Model, json!(true), true),
            money("owed", 0.9, false),
        ])
        .unwrap();
        assert_eq!(
            (by_hand.reason, by_hand.trust),
            (NeedReason::Pay, NeedTrust::Check)
        );
        let promotion = [
            stream(NeedBasis::Model, "newsletters", true),
            money("owed", 0.9, false),
        ];
        assert_eq!(raised(&promotion), None);
    }

    #[test]
    fn a_later_message_in_a_receipts_conversation() {
        // The conversation was placed by an earlier message: this stream
        // answer did not act, but the policy accepts it.
        let need = raised(&[
            stream(NeedBasis::Model, "receipts", false),
            money("owed", 0.9, false),
        ])
        .unwrap();
        assert_eq!(need.reason, NeedReason::Pay);
        let mut unsure = stream(NeedBasis::Model, "receipts", false);
        unsure.confidence = Some(0.6);
        let asks = answer(triage::ASKS_ACTION, NeedBasis::Model, json!(true), true);
        assert_eq!(
            raised(&[unsure.clone(), asks.clone()]),
            None,
            "mail no answer sorted needs a deadline"
        );
        let need = raised(&[
            unsure,
            asks,
            answer(triage::HAS_DEADLINE, NeedBasis::Model, json!(true), true),
        ])
        .unwrap();
        assert_eq!(need.reason, NeedReason::Do);
    }

    #[test]
    fn automated_mail_asks_only_with_a_deadline() {
        // A calendar invite or a support notice the model reads as asking
        // something, with no time limit: nothing.
        let invite = [
            stream(NeedBasis::Model, "updates", true),
            answer(triage::ASKS_ACTION, NeedBasis::Model, json!(true), true),
            answer(triage::HAS_DEADLINE, NeedBasis::Model, json!(false), true),
        ];
        assert_eq!(raised(&invite), None);
        // A delivery to reschedule by a date: something to do.
        let reschedule = [
            stream(NeedBasis::Model, "updates", true),
            answer(triage::ASKS_ACTION, NeedBasis::Model, json!(true), true),
            answer(triage::HAS_DEADLINE, NeedBasis::Model, json!(true), true),
        ];
        assert_eq!(raised(&reschedule).unwrap().reason, NeedReason::Do);
    }

    #[test]
    fn list_mail_never_needs_anything() {
        let newsletter = [
            stream(NeedBasis::Model, "newsletters", true),
            answer(triage::NEEDS_REPLY, NeedBasis::Model, json!(true), true),
            answer(triage::ASKS_ACTION, NeedBasis::Model, json!(true), true),
            answer(triage::HAS_DEADLINE, NeedBasis::Model, json!(true), true),
        ];
        assert_eq!(raised(&newsletter), None);
    }

    #[test]
    fn suspicious_mail_is_only_a_warning() {
        let need = raised(&[
            answer(triage::ASKS_ACTION, NeedBasis::Model, json!(true), true),
            answer(triage::SUSPICIOUS, NeedBasis::Model, json!(true), true),
        ])
        .unwrap();
        assert_eq!(need.reason, NeedReason::Review);
        assert_eq!(need.question, NeedQuestion::Suspicious);
        assert_eq!(need.trust, NeedTrust::Sure);
    }

    #[test]
    fn answers_that_did_not_act_raise_nothing_sure() {
        // The retired rules' answers, marked not acted on upgrade.
        assert_eq!(
            raised(&[
                answer(triage::NEEDS_REPLY, NeedBasis::Rules, json!(true), false),
                answer(triage::SUSPICIOUS, NeedBasis::Rules, json!(true), false),
            ]),
            None
        );
        assert_eq!(
            raised(&[answer(
                triage::NEEDS_REPLY,
                NeedBasis::Rules,
                json!(false),
                true
            )]),
            None
        );
    }

    #[test]
    fn a_confident_model_suggestion_is_a_question() {
        let need = raised(&[
            stream(NeedBasis::Model, "primary", true),
            answer(triage::NEEDS_REPLY, NeedBasis::Model, json!(true), false),
        ])
        .unwrap();
        assert_eq!(
            (need.reason, need.trust, need.basis),
            (NeedReason::Review, NeedTrust::Check, NeedBasis::Model)
        );
        let mut unsure = answer(triage::NEEDS_REPLY, NeedBasis::Model, json!(true), false);
        unsure.confidence = Some(0.6);
        assert_eq!(
            raised(&[stream(NeedBasis::Model, "primary", true), unsure]),
            None
        );
    }

    fn confident(mut a: Answer, confidence: f64) -> Answer {
        a.confidence = Some(confidence);
        a
    }

    /// A shipping notice with a list header ending "Questions? Contact our
    /// support team.", in a conversation a person started: the facts' acted
    /// "no", and a model that reads the question mark as one for the
    /// recipient.
    fn notification_answers() -> Vec<Answer> {
        vec![
            stream(NeedBasis::Model, "primary", false),
            with_reason(
                confident(
                    answer(triage::NEEDS_REPLY, NeedBasis::Rules, json!(false), true),
                    0.99,
                ),
                "facts.list_mail",
                "List-Unsubscribe",
            ),
            confident(
                answer(triage::NEEDS_REPLY, NeedBasis::Model, json!(true), true),
                0.95,
            ),
            confident(
                answer(triage::ASKS_ACTION, NeedBasis::Model, json!(true), false),
                0.82,
            ),
        ]
    }

    #[test]
    fn the_facts_no_beats_an_acted_model_yes() {
        // The model's "yes" to a reply is overruled; its unaccepted ask is
        // still a question.
        let need = raised(&notification_answers()).unwrap();
        assert_eq!(
            (need.reason, need.trust, need.question),
            (
                NeedReason::Review,
                NeedTrust::Check,
                NeedQuestion::AsksAction
            )
        );
        let mut answers = notification_answers();
        answers.pop();
        assert_eq!(raised(&answers), None, "a notification needs no reply");
    }

    #[test]
    fn a_suggestion_against_a_settled_answer_is_no_question() {
        let answers = [
            stream(NeedBasis::Model, "primary", true),
            answer(triage::NEEDS_REPLY, NeedBasis::Rules, json!(false), true),
            confident(
                answer(triage::NEEDS_REPLY, NeedBasis::Model, json!(true), false),
                0.84,
            ),
        ];
        assert_eq!(raised(&answers), None);
    }

    #[test]
    fn a_model_suggestion_alone_is_still_a_question() {
        let need = raised(&[
            stream(NeedBasis::Model, "primary", true),
            confident(
                answer(triage::ASKS_ACTION, NeedBasis::Model, json!(true), false),
                0.84,
            ),
        ])
        .unwrap();
        assert_eq!(
            (need.reason, need.trust, need.question),
            (
                NeedReason::Review,
                NeedTrust::Check,
                NeedQuestion::AsksAction
            )
        );
    }

    fn need(reason: NeedReason) -> Need {
        Need {
            message: MessageId::new(),
            reason,
            trust: NeedTrust::Sure,
            basis: NeedBasis::Rules,
            question: NeedQuestion::NeedsReply,
            decision: None,
            due_on: None,
            urgent: false,
        }
    }

    fn cause(settled: &Settled) -> Option<ClearCause> {
        match settled {
            Settled::Open(_) => None,
            Settled::Cleared(_, cause) => Some(*cause),
        }
    }

    #[test]
    fn the_clearing_table() {
        use NeedReason::{Do, Pay, Reply, Review};
        let replied = Since {
            replied: true,
            ..Since::default()
        };
        let archived = Since {
            archived: true,
            ..Since::default()
        };
        let trashed = Since {
            trashed: true,
            ..Since::default()
        };
        let expect = [
            (
                replied,
                [
                    Some(ClearCause::Replied),
                    Some(ClearCause::Replied),
                    None,
                    None,
                ],
            ),
            (
                archived,
                [
                    Some(ClearCause::Archived),
                    None,
                    None,
                    Some(ClearCause::Archived),
                ],
            ),
            (trashed, [Some(ClearCause::Trashed); 4]),
            (Since::default(), [None; 4]),
        ];
        for (since, causes) in expect {
            for (reason, expected) in [Reply, Do, Pay, Review].into_iter().zip(causes) {
                assert_eq!(
                    cause(&settle(need(reason), since)),
                    expected,
                    "{reason:?} {since:?}"
                );
            }
        }
    }

    #[test]
    fn a_verdict_wins() {
        let done = Since {
            verdict: Some(NeedVerdict::Done),
            ..Since::default()
        };
        assert_eq!(
            cause(&settle(need(NeedReason::Pay), done)),
            Some(ClearCause::Done)
        );
        let confirmed = Since {
            verdict: Some(NeedVerdict::Confirmed),
            ..Since::default()
        };
        let mut question = need(NeedReason::Review);
        question.trust = NeedTrust::Check;
        match settle(question, confirmed) {
            Settled::Open(need) => {
                assert_eq!(
                    (need.reason, need.trust, need.basis),
                    (NeedReason::Reply, NeedTrust::Sure, NeedBasis::Member)
                );
            }
            Settled::Cleared(..) => panic!("a confirmed question stays open"),
        }
    }

    #[test]
    fn is_it_paid() {
        let mut question = need(NeedReason::Pay);
        question.trust = NeedTrust::Check;
        question.question = NeedQuestion::Money;
        let paid = Since {
            verdict: Some(NeedVerdict::AlreadyPaid),
            ..Since::default()
        };
        assert_eq!(
            cause(&settle(question.clone(), paid)),
            Some(ClearCause::Done)
        );
        let to_pay = Since {
            verdict: Some(NeedVerdict::StillToPay),
            ..Since::default()
        };
        match settle(question, to_pay) {
            Settled::Open(need) => assert_eq!(
                (need.reason, need.trust, need.basis),
                (NeedReason::Pay, NeedTrust::Sure, NeedBasis::Member)
            ),
            Settled::Cleared(..) => panic!("still to pay stays open"),
        }
    }
}
