//! What a conversation needs from its inbox's members (attention,
//! campbooks-cloud#39).
//!
//! At most one need per conversation, kept in `conversations.needs`:
//!
//! - **Raised** ([`raise_with`], pure) from the decide pass's answers about the
//!   newest inbound message that is not in spam or the trash: `pay` when the
//!   money answer says a payment is still owed, always as a question ("Is it
//!   paid?", campbooks-cloud#167: Campbooks cannot see the bank); `reply` or
//!   `do` from answers the policy let act; `review` for a suspicious message
//!   (a warning) or, when a model answers, for a suggestion put to the
//!   member as a question. Mail sorted to Receipts needs nothing but a
//!   payment still owed: a receipt or a payment confirmation needs nothing.
//!   The facts only ever say "no" (ADR 0018), so a sure need comes from the
//!   model; a facts "no" still wins over a model's "yes".
//! - **Settled** ([`settle`], pure) against what happened since and the
//!   time now ([`Clock`]): a reply clears Reply and Do, an archive clears
//!   Reply and Review, the trash or spam (the conversation's, or the need's
//!   own message's) clears everything, and a member's verdict (Done, Not
//!   needed, a confirmed question, Already paid, Still to pay) wins. Then
//!   the clock (campbooks-cloud#275): a need whose lapse time has passed is
//!   cleared as lapsed (never a bill or a warning), and an undated `do` or
//!   `reply` whose message left the needs window, that no member confirmed,
//!   becomes a question: "Still to do?", "Still needs a reply?".
//! - **Waiting** (campbooks-cloud#293): when the newest inbound message asks
//!   nothing of the member but someone else has the next move (the model's
//!   `next_move` and `awaited` answers, the person only ever one the headers
//!   name), the conversation waits on that person ([`NeedReason::Waiting`],
//!   shown nowhere in the queue). At its nudge time ([`nudge_time`]: the
//!   working day after the date Scout read for them, else
//!   `CAMPBOOKS_CONVERSATIONS__WAIT__WORKING_DAYS` working days) it becomes a
//!   Nudge; mail from them reaching the inbox clears it as `heard_back`; the
//!   member writing to them re-arms it from the send. Archiving leaves it.
//! - **Refreshed** ([`refresh`]) wherever those inputs change, publishing
//!   `conversations.need_changed` with a growing revision when the need did,
//!   and writing when it next changes with time alone
//!   (`conversations.need_clocks`), which `conversations.wake_needs` acts on.
//!
//! Mail text is never an instruction here. A need's due date and lapse time
//! are what Scout read from its message's email (`read_message`, the
//! `complete` role), kept with the message in `message_reads` and copied
//! onto the need; answers stored before ADR 0018 may carry the sender's
//! sentence, only ever shown, quoted.

use campbooks_ai::decide::signals::{
    email::{Envelope, Party, Recipient, triage},
    facts::{self, dmarc_refused, sender_authenticated, sender_checks_failed},
    moves::{self, NextMove},
};
use campbooks_kernel::{Error, EventBus, Result, tenancy::UserId};
use chrono::{DateTime, Datelike, NaiveDate, TimeDelta, Utc, Weekday};
use serde::{Deserialize, Serialize};
use sqlx::{PgConnection, Postgres, Transaction};
use utoipa::ToSchema;

use super::{
    conversation::ConversationSummary,
    decide::{MessageParts, Source, envelope},
    policy::Policy,
};
use crate::{
    config::DatedSettings,
    events::{NeedChanged, NeedFacts},
    ingest::Headers,
    model::{
        Address, ClearCause, ConversationId, DecisionId, InboxId, MessageId, NeedBasis,
        NeedQuestion, NeedReason, NeedTrust, NeedVerdict, Verb,
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
/// The confidence from which the model's `next_move` raises a wait the
/// policy does not accept yet (campbooks-cloud#293): put as a question.
const WAIT_FLOOR: f64 = 0.8;
/// The facts that say a message is list, automatic or no-reply mail: it
/// waits on nobody.
const MACHINE_MAIL: [&str; 3] = [facts::LIST_MAIL, facts::AUTOMATIC, facts::NO_REPLY_SENDER];

/// How needs are raised: the window of mail that may raise one, and the
/// policy (which stream answers count, and its floor for questions).
#[derive(Debug, Clone)]
pub struct NeedSettings {
    /// Only inbound messages sent within this long ago raise a need.
    pub window: TimeDelta,
    /// Which answers may act.
    pub policy: Policy,
    /// Whether a need that opens on a flagged message queues Scout's read of
    /// its email (`conversations.read_message`): only when the `complete` role may
    /// receive mail on this server.
    pub summaries: bool,
    /// Working days a wait lasts before Scout nudges, when its email gives
    /// no date (`CAMPBOOKS_CONVERSATIONS__WAIT__WORKING_DAYS`).
    pub wait_days: u32,
    /// Whether a new wait may be raised: only while the action catalogue is
    /// switched on (`CAMPBOOKS_CONVERSATIONS__ACTIONS__ENABLED`). Off, an
    /// open wait keeps its course; none opens.
    pub waits: bool,
    /// Which messages are flagged for Scout's read of their dates
    /// (`CAMPBOOKS_CONVERSATIONS__DATED__*`).
    pub dated: DatedSettings,
}

impl NeedSettings {
    /// A window of `days`, under `policy`.
    pub fn new(days: u32, policy: &Policy) -> Self {
        Self {
            window: TimeDelta::days(i64::from(days)),
            policy: policy.clone(),
            summaries: false,
            wait_days: crate::config::DEFAULT_WAIT_WORKING_DAYS,
            waits: true,
            dated: DatedSettings::default(),
        }
    }

    /// The same settings, flagging messages for a read as `dated` says.
    #[must_use]
    pub const fn with_dated(mut self, dated: DatedSettings) -> Self {
        self.dated = dated;
        self
    }

    /// The same settings, raising new waits only when `on`.
    #[must_use]
    pub const fn with_waits(mut self, on: bool) -> Self {
        self.waits = on;
        self
    }

    /// The same settings, a wait lasting `days` working days.
    #[must_use]
    pub const fn with_wait_days(mut self, days: u32) -> Self {
        self.wait_days = days;
        self
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

/// One stored answer, as [`raise_with`] reads it.
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
    /// The options it was asked with (`awaited`'s carry their
    /// descriptions); empty when unknown.
    pub options: serde_json::Value,
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
    pub(crate) fn is_true(&self) -> bool {
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
    /// When what it asks stops being possible (none for a bill or a
    /// warning, which never lapse).
    pub lapses_at: Option<DateTime<Utc>>,
    /// Whether it is for today.
    pub urgent: bool,
    /// Who it waits on (a `waiting` or `nudge` need only).
    pub awaited: Option<Awaited>,
    /// When Scout nudges (a `waiting` or `nudge` need only).
    pub nudge_at: Option<DateTime<Utc>>,
    /// How many times the member wrote to the person waited on since.
    pub nudges: i32,
}

/// The person a wait is on, as the email's headers gave them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Awaited {
    /// Their display name; empty when the headers gave none.
    pub name: String,
    /// Their address, trimmed.
    pub address: String,
}

impl Awaited {
    /// The address as hearing back compares it: lower-cased.
    fn key(&self) -> String {
        self.address.trim().to_lowercase()
    }
}

/// What Scout read about a message's dates (`message_reads`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Dates {
    /// By when the recipient is asked to act.
    pub due_on: Option<NaiveDate>,
    /// When what it asks stops being possible.
    pub lapses_at: Option<DateTime<Utc>>,
}

impl Need {
    /// Whether it can lapse: anything but a bill (it stays until a member
    /// answers it), a warning (it keeps its own rules) and "Worth a look"
    /// (a rule the member taught keeps it until it is opened: nothing the
    /// mail says undoes that).
    pub fn can_lapse(&self) -> bool {
        !matches!(
            self.reason,
            NeedReason::Pay | NeedReason::WorthALook | NeedReason::Waiting | NeedReason::Nudge
        ) && !matches!(
            self.question,
            NeedQuestion::Money
                | NeedQuestion::Suspicious
                | NeedQuestion::WorthALook
                | NeedQuestion::NextMove
        )
    }

    /// Whether it waits on someone: `waiting`, or `nudge` once its time
    /// passed. Never dropped by age, never lapsing (campbooks-cloud#293).
    pub const fn is_wait(&self) -> bool {
        matches!(self.reason, NeedReason::Waiting | NeedReason::Nudge)
    }

    /// The wait with its nudge time worked out: from the member's newest
    /// message to the person (`sends`, which also counts the nudges), else
    /// from its own message (`sent`) and the date Scout read for the person
    /// (`due_on`, as [`Self::dated`] set it). Its due date is the nudge
    /// time's date. Anything but a wait is returned as it is.
    #[must_use]
    pub fn armed(mut self, sent: DateTime<Utc>, sends: Option<Sends>, wait_days: u32) -> Self {
        if !self.is_wait() {
            return self;
        }
        let (at, nudges) = match sends {
            Some(sends) => (after_working_days(sends.last, wait_days), sends.count),
            None => (nudge_time(sent, self.due_on, wait_days), 0),
        };
        self.nudge_at = Some(at);
        self.nudges = nudges;
        self.due_on = Some(at.date_naive());
        self.lapses_at = None;
        self
    }

    /// The need with its message's `dates`: the due date, and the lapse
    /// time when it can lapse.
    #[must_use]
    pub fn dated(mut self, dates: Dates) -> Self {
        self.due_on = dates.due_on;
        self.lapses_at = dates.lapses_at.filter(|_| self.can_lapse());
        self
    }

    /// Whether it is an ask that time alone turns into "Still needed?": a
    /// sure `do` or `reply` with no date of its own that no member
    /// confirmed.
    fn may_go_stale(&self) -> bool {
        matches!(self.reason, NeedReason::Do | NeedReason::Reply)
            && self.trust == NeedTrust::Sure
            && self.basis != NeedBasis::Member
            && self.due_on.is_none()
            && self.lapses_at.is_none()
    }
}

/// The time a need is settled against: now, and the needs window (an
/// undated ask whose message is older than it is asked again).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Clock {
    /// Now.
    pub now: DateTime<Utc>,
    /// The needs window.
    pub window: TimeDelta,
    /// Working days a wait lasts when its email gives no date.
    pub wait_days: u32,
}

impl Clock {
    /// The clock at this moment, with `settings`' window.
    pub fn now(settings: &NeedSettings) -> Self {
        Self {
            now: Utc::now(),
            window: settings.window,
            wait_days: settings.wait_days,
        }
    }
}

/// Whether `day` is a working day: Monday to Friday.
fn working_day(day: NaiveDate) -> bool {
    !matches!(day.weekday(), Weekday::Sat | Weekday::Sun)
}

/// The first working day after `day`.
fn next_working_day(day: NaiveDate) -> NaiveDate {
    let mut next = day;
    while let Some(after) = next.succ_opt() {
        next = after;
        if working_day(next) {
            break;
        }
    }
    next
}

/// `from` plus `days` working days (Monday to Friday, by UTC date), at the
/// same time of day.
pub fn after_working_days(from: DateTime<Utc>, days: u32) -> DateTime<Utc> {
    let mut day = from.date_naive();
    for _ in 0..days {
        day = next_working_day(day);
    }
    day.and_time(from.time()).and_utc()
}

/// When Scout nudges about a message sent at `sent` (campbooks-cloud#293):
/// the working day after `due_on` (the date Scout read for the person
/// waited on), else `wait_days` working days after it; at the time of day it
/// was sent.
pub fn nudge_time(sent: DateTime<Utc>, due_on: Option<NaiveDate>, wait_days: u32) -> DateTime<Utc> {
    match due_on {
        Some(due) => next_working_day(due).and_time(sent.time()).and_utc(),
        None => after_working_days(sent, wait_days),
    }
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

/// The need the answers about `message` raise, if any, under `settings`,
/// without the email's people: never a wait (see [`raise_with`]).
/// `look_ok` is whether "Worth a look" may be raised for it
/// ([`may_be_looked_at`]): a stored highlight on mail that is not
/// authenticated, or looks suspicious, raises nothing.
#[cfg(test)]
pub fn raise(
    message: MessageId,
    answers: &[Answer],
    settings: &NeedSettings,
    look_ok: bool,
) -> Option<Need> {
    raise_with(message, answers, None, settings, look_ok)
}

/// Whether the policy accepted the model's `next_move` answer, saying
/// someone other than the recipient has the move: a model's own "needs a
/// reply" or "asks for an action" then raises nothing (campbooks-cloud#293).
/// The shipped policy accepts none yet.
fn moved_on(answers: &[Answer]) -> bool {
    answers.iter().any(|a| {
        a.question == moves::question::NEXT_MOVE
            && a.basis == NeedBasis::Model
            && a.acted
            && a.value.as_str() != Some(NextMove::Recipient.as_str())
    })
}

/// The need the answers about `message` raise, if any, under `settings`;
/// with its `people` (the envelope's sender and recipients, and whose inbox
/// it is), a wait when nothing else is raised.
#[allow(clippy::too_many_lines)]
pub fn raise_with(
    message: MessageId,
    answers: &[Answer],
    people: Option<&Envelope>,
    settings: &NeedSettings,
    look_ok: bool,
) -> Option<Need> {
    let check_floor = settings.check_floor();
    let need = |reason, trust, basis, question, decision: &Answer| Need {
        message,
        reason,
        trust,
        basis,
        question,
        decision: Some(decision.id),
        due_on: None,
        lapses_at: None,
        urgent: false,
        awaited: None,
        nudge_at: None,
        nudges: 0,
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
    // The policy says someone else has the move: a model's own ask is not
    // the member's (a taught rule's still is).
    let moved_on = moved_on(answers);
    let ours = |a: &&Answer| !(moved_on && a.basis == NeedBasis::Model);
    let asks = accepted(answers, triage::ASKS_ACTION).filter(ours);
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
    } else if let Some(reply) = accepted(answers, triage::NEEDS_REPLY).filter(ours) {
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
        .filter(|(id, _)| !moved_on && !settled_no(answers, id))
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
    // Anything that asks something of the member comes first, then what a
    // rule the member taught keeps in front of them, then a wait.
    raised
        .or_else(|| highlighted(message, answers).filter(|_| look_ok))
        .or_else(|| {
            people
                .filter(|_| settings.waits)
                .and_then(|people| waiting(message, answers, people, settings))
        })
}

/// The wait the answers about `message` raise when nothing else is raised
/// (campbooks-cloud#293): on the sender, when the model says the sender has
/// the next move; on the person its `awaited` answer names, when someone
/// else has. Only for mail the facts do not mark as list, automatic or
/// no-reply mail, sorted to Primary or not sorted by an accepted answer. A
/// `next_move` the policy accepts is `sure`; one it does not, a question,
/// from [`WAIT_FLOOR`]. The person is only ever one of the email's own
/// people, and a stored `pN` is read only as the person its option named
/// when it was asked.
fn waiting(
    message: MessageId,
    answers: &[Answer],
    people: &Envelope,
    settings: &NeedSettings,
) -> Option<Need> {
    let machine = answers.iter().any(|a| {
        a.basis == NeedBasis::Rules
            && a.reasons
                .iter()
                .any(|r| MACHINE_MAIL.contains(&r.rule.as_str()))
    });
    if machine
        || !matches!(
            sorted_stream(answers, &settings.policy),
            None | Some(PRIMARY)
        )
    {
        return None;
    }
    let model = |question: &str| {
        answers
            .iter()
            .find(|a| a.question == question && a.basis == NeedBasis::Model)
    };
    let next = model(moves::question::NEXT_MOVE)?;
    if !next.acted && !next.confidence.is_some_and(|c| c >= WAIT_FLOOR) {
        return None;
    }
    let person = match NextMove::parse(next.value.as_str()?)? {
        // A sender that is a machine (a no-reply or service address) is
        // never waited on.
        NextMove::Sender if !facts::is_machine_sender(people) => people.from.clone(),
        NextMove::SomeoneElse => {
            let awaited = model(moves::question::AWAITED)?;
            let label = awaited.value.as_str()?;
            let person = moves::awaited_party(people, label)?;
            let asked_about = option_description(&awaited.options, label)?;
            moves::describes(asked_about, &person).then_some(person)?
        }
        NextMove::Sender | NextMove::Recipient | NextMove::Nobody => return None,
    };
    let Party { name, address } = person;
    let address = address.trim().to_owned();
    // Never a wait on the inbox itself: Gmail and Zoho file mail the inbox
    // sent itself as received, so its sender can be the inbox.
    let ours = people.recipient.as_ref().is_some_and(|inbox| {
        inbox
            .addresses
            .iter()
            .any(|own| own.trim().eq_ignore_ascii_case(&address))
    });
    if address.is_empty() || ours {
        return None;
    }
    Some(Need {
        message,
        reason: NeedReason::Waiting,
        trust: if next.acted {
            NeedTrust::Sure
        } else {
            NeedTrust::Check
        },
        basis: NeedBasis::Model,
        question: NeedQuestion::NextMove,
        decision: Some(next.id),
        due_on: None,
        lapses_at: None,
        urgent: false,
        awaited: Some(Awaited {
            name: name.trim().to_owned(),
            address,
        }),
        nudge_at: None,
        nudges: 0,
    })
}

/// The description stored with option `label` of a choice's `options`
/// (`[{label, description}]`), if it was stored.
fn option_description<'a>(options: &'a serde_json::Value, label: &str) -> Option<&'a str> {
    options
        .as_array()?
        .iter()
        .find(|o| o.get("label").and_then(serde_json::Value::as_str) == Some(label))?
        .get("description")?
        .as_str()
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
            lapses_at: None,
            urgent: false,
            awaited: None,
            nudge_at: None,
            nudges: 0,
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

/// The question an ask was raised by: a "Still needed?" question stands for
/// the ask it was raised as; every other question is its own.
fn asked(question: NeedQuestion) -> NeedQuestion {
    match question {
        NeedQuestion::StillToDo => NeedQuestion::AsksAction,
        NeedQuestion::StillNeedsReply => NeedQuestion::NeedsReply,
        NeedQuestion::NeedsReply
        | NeedQuestion::AsksAction
        | NeedQuestion::Suspicious
        | NeedQuestion::Money
        | NeedQuestion::WorthALook
        | NeedQuestion::NextMove => question,
    }
}

/// Whether a taught rule now says the need raised by `question` is not
/// needed: "never", or a false answer to that question. A warning about
/// suspicious mail is never cleared this way.
fn rule_clears(answers: &[Answer], question: NeedQuestion) -> bool {
    // A "Still needed?" question stands for the ask it was raised as.
    let question = asked(question);
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
    /// When the need's message was sent (`None` once it is gone).
    pub sent_at: Option<DateTime<Utc>>,
    /// The message was opened (read) by someone, here or at the provider.
    pub opened: bool,
    /// Mail from the person a wait is on reached the inbox after it.
    pub heard_back: bool,
    /// The member's messages to the person a wait is on since it, if any.
    pub sends: Option<Sends>,
}

/// The member's messages to the person a wait is on, since its message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sends {
    /// When the newest was sent.
    pub last: DateTime<Utc>,
    /// How many.
    pub count: i32,
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

/// Settle `need` against what happened since and the time now (the
/// clearing table of the design).
pub fn settle(mut need: Need, since: Since, clock: Clock) -> Settled {
    // A wait's nudge time: from its message, or the member's newest message
    // to the person since.
    if let Some(sent) = since.sent_at {
        need = need.armed(sent, since.sends, clock.wait_days);
    }
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
                    NeedQuestion::AsksAction | NeedQuestion::StillToDo => NeedReason::Do,
                    NeedQuestion::Money => NeedReason::Pay,
                    NeedQuestion::NeedsReply
                    | NeedQuestion::Suspicious
                    | NeedQuestion::StillNeedsReply => NeedReason::Reply,
                    NeedQuestion::WorthALook => NeedReason::WorthALook,
                    // "Yes, I'm waiting": still a wait.
                    NeedQuestion::NextMove => need.reason,
                };
                need.question = asked(need.question);
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
    // They wrote: nothing to wait for. Archiving never ends a wait.
    if since.heard_back && need.is_wait() {
        return Settled::Cleared(need, ClearCause::HeardBack);
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
    // What it asks is no longer possible (the link expired, the offer
    // closed): whoever stood behind it, there is nothing left to do.
    if need.can_lapse() && need.lapses_at.is_some_and(|at| at <= clock.now) {
        return Settled::Cleared(need, ClearCause::Lapsed);
    }
    // An undated ask from longer ago than the window: asked again, as a
    // question, until a member says it still is (campbooks-cloud#275).
    if need.may_go_stale()
        && since
            .sent_at
            .is_some_and(|sent| sent + clock.window <= clock.now)
    {
        need.question = if need.reason == NeedReason::Do {
            NeedQuestion::StillToDo
        } else {
            NeedQuestion::StillNeedsReply
        };
        need.reason = NeedReason::Review;
        need.trust = NeedTrust::Check;
    }
    // A wait whose nudge time passed is a Nudge; written to since, a wait
    // again.
    if need.is_wait() {
        need.reason = if need.nudge_at.is_some_and(|at| at <= clock.now) {
            NeedReason::Nudge
        } else {
            NeedReason::Waiting
        };
    }
    Settled::Open(need)
}

/// When `settled`, if open, next changes with time alone: its lapse time,
/// or when its message (sent at `sent_at`) leaves the window while it is an
/// undated ask, whichever comes first and is still ahead of `clock`.
pub fn next_wake(
    settled: &Settled,
    sent_at: Option<DateTime<Utc>>,
    clock: Clock,
) -> Option<DateTime<Utc>> {
    let Settled::Open(need) = settled else {
        return None;
    };
    let lapse = need.lapses_at.filter(|_| need.can_lapse());
    let stale = sent_at
        .filter(|_| need.may_go_stale())
        .and_then(|sent| sent.checked_add_signed(clock.window));
    let nudge = need.nudge_at.filter(|_| need.reason == NeedReason::Waiting);
    [lapse, stale, nudge]
        .into_iter()
        .flatten()
        .filter(|at| *at > clock.now)
        .min()
}

/// The stored row.
#[derive(Debug, sqlx::FromRow)]
struct Row {
    message_id: MessageId,
    reason: String,
    trust: String,
    basis: String,
    question: String,
    decision_id: Option<DecisionId>,
    due_on: Option<NaiveDate>,
    lapses_at: Option<DateTime<Utc>>,
    urgent: bool,
    state: String,
    cleared_because: Option<String>,
    verdict: Option<String>,
    verdict_message_id: Option<MessageId>,
    verdict_by: Option<UserId>,
    latest_inbound_id: Option<MessageId>,
    awaited_name: Option<String>,
    awaited_address: Option<String>,
    nudge_at: Option<DateTime<Utc>>,
    nudges: i32,
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
            lapses_at: self.lapses_at,
            urgent: self.urgent,
            awaited: self.awaited_address.clone().map(|address| Awaited {
                name: self.awaited_name.clone().unwrap_or_default(),
                address,
            }),
            nudge_at: self.nudge_at,
            nudges: self.nudges,
        })
    }

    fn verdict_on(&self, message: MessageId) -> Result<Option<NeedVerdict>> {
        match (&self.verdict, self.verdict_message_id) {
            (Some(verdict), Some(on)) if on == message => Ok(Some(verdict.parse()?)),
            _ => Ok(None),
        }
    }
}

/// The conversation's stored need, locked. A runtime query, so the columns
/// added since need no entry in the `.sqlx` offline cache.
async fn current(conn: &mut PgConnection, conversation: ConversationId) -> Result<Option<Row>> {
    Ok(sqlx::query_as::<_, Row>(
        "SELECT message_id, reason, trust, basis, question, decision_id, due_on, lapses_at,
                urgent, state, cleared_because, verdict, verdict_message_id, verdict_by,
                latest_inbound_id, awaited_name, awaited_address, nudge_at, nudges
           FROM conversations.needs WHERE conversation_id = $1 FOR UPDATE",
    )
    .bind(conversation)
    .fetch_optional(conn)
    .await?)
}

/// A row of `message_reads` as [`dates`] reads it: layer, lapse time, due date
/// and asks.
type ReadDates = (
    String,
    Option<DateTime<Utc>>,
    Option<NaiveDate>,
    serde_json::Value,
);

/// What Scout read about `message`'s dates, if it has read them, for a need
/// of `reason`: the lapse time and the due date its read gave, except that a
/// `do` or `pay` need takes its due date from the first ask of the read with
/// the same verb that has one (campbooks-cloud#42), and the other reasons
/// from the read's own `due_on`.
async fn dates(conn: &mut PgConnection, message: MessageId, reason: NeedReason) -> Result<Dates> {
    let rows: Vec<ReadDates> = sqlx::query_as(
        "SELECT layer, lapses_at, due_on, asks FROM conversations.message_reads
              WHERE message_id = $1 AND state = 'read' ORDER BY layer DESC",
    )
    .bind(message)
    .fetch_all(conn)
    .await?;
    // The model's read gave the need its lapse time and, for a reply, a
    // review or a wait, its due date.
    let model = rows.iter().find(|(layer, ..)| layer == "model");
    let verb = match reason {
        NeedReason::Do => Some(Verb::Do),
        NeedReason::Pay => Some(Verb::Pay),
        _ => None,
    };
    let due_on = match verb {
        // The model's asks first, then the sender's own data's.
        Some(verb) => rows
            .iter()
            .filter(|(layer, ..)| layer == "model")
            .chain(rows.iter().filter(|(layer, ..)| layer != "model"))
            .find_map(|(.., asks)| first_due(asks, verb)),
        None => model.and_then(|(_, _, due_on, _)| *due_on),
    };
    Ok(Dates {
        due_on,
        lapses_at: model.and_then(|(_, lapses_at, ..)| *lapses_at),
    })
}

/// The date the first ask of `asks` with `verb` and a due is due.
fn first_due(asks: &serde_json::Value, verb: Verb) -> Option<NaiveDate> {
    asks.as_array()?
        .iter()
        .filter(|ask| ask.get("verb").and_then(serde_json::Value::as_str) == Some(verb.as_str()))
        .find_map(|ask| {
            let due = ask.get("due")?.as_str()?;
            super::read::When::parse(due).map(super::read::When::date)
        })
}

/// Write when conversation `id`'s open need next changes with time alone,
/// or forget it (`None`). After the needs row is written and locked, as
/// every caller of [`refresh`] locks the needs row first.
async fn set_clock(
    tx: &mut Transaction<'_, Postgres>,
    workspace_id: sqlx::types::Uuid,
    id: ConversationId,
    wake: Option<DateTime<Utc>>,
) -> Result<()> {
    match wake {
        Some(at) => {
            sqlx::query(
                "INSERT INTO conversations.need_clocks (conversation_id, workspace_id, wake_at)
                 VALUES ($1, $2, $3)
                 ON CONFLICT (conversation_id) DO UPDATE SET wake_at = EXCLUDED.wake_at",
            )
            .bind(id)
            .bind(workspace_id)
            .bind(at)
            .execute(&mut **tx)
            .await?;
        }
        None => {
            sqlx::query("DELETE FROM conversations.need_clocks WHERE conversation_id = $1")
                .bind(id)
                .execute(&mut **tx)
                .await?;
        }
    }
    Ok(())
}

/// The answers stored about `message`. A model's answer counts only when it
/// was given while the model passed its answers check
/// (`campbooks_ai::decide::check::STAMP`): the rest are kept, never read.
pub(crate) async fn answers(conn: &mut PgConnection, message: MessageId) -> Result<Vec<Answer>> {
    // A runtime query, so the column read since (`options`) needs no entry
    // in the `.sqlx` offline cache.
    let rows: Vec<AnswerRow> = sqlx::query_as(
        "SELECT id, question, source, value, confidence, acted, reasons, options
           FROM conversations.decisions
          WHERE message_id = $1 AND (source <> 'model' OR model_version LIKE 'checked:%')",
    )
    .bind(message)
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
                options: row.options,
            })
        })
        .collect()
}

/// A row of [`answers`].
#[derive(sqlx::FromRow)]
struct AnswerRow {
    id: DecisionId,
    question: String,
    source: String,
    value: serde_json::Value,
    confidence: Option<f64>,
    acted: bool,
    reasons: serde_json::Value,
    options: serde_json::Value,
}

/// A row of [`people`]: sender, recipients, headers, and the inbox's
/// address and name.
type PeopleRow = (
    serde_json::Value,
    serde_json::Value,
    serde_json::Value,
    String,
    Option<String>,
);

/// The people of `message` as the decide pass read them: its sender,
/// recipients and list headers, and whose inbox it is. `None` once it is
/// gone.
async fn people(conn: &mut PgConnection, message: MessageId) -> Result<Option<Envelope>> {
    let row: Option<PeopleRow> = sqlx::query_as(
        "SELECT m.sender, m.recipients, m.headers, i.address, i.display_name
               FROM conversations.messages m
               JOIN conversations.inboxes i ON i.id = m.inbox_id
              WHERE m.id = $1",
    )
    .bind(message)
    .fetch_optional(conn)
    .await?;
    let Some((sender, recipients, headers, inbox_address, inbox_name)) = row else {
        return Ok(None);
    };
    let decode = |e: serde_json::Error| Error::internal(super::redact::json(&e));
    let from: Address = serde_json::from_value(sender).map_err(decode)?;
    let recipients: StoredRecipients = serde_json::from_value(recipients).map_err(decode)?;
    let headers: Headers = serde_json::from_value(headers).map_err(decode)?;
    let mut people = envelope(MessageParts {
        from: &from,
        to: &recipients.to,
        cc: &recipients.cc,
        reply_to: &recipients.reply_to,
        subject: "",
        headers: &headers,
        attachments: &[],
        body: "",
    });
    people.recipient = Some(Recipient {
        name: inbox_name.unwrap_or_default(),
        addresses: vec![inbox_address],
        shared: false,
    });
    Ok(Some(people))
}

/// `messages.recipients`.
#[derive(Deserialize)]
struct StoredRecipients {
    #[serde(default)]
    to: Vec<Address>,
    #[serde(default)]
    cc: Vec<Address>,
    #[serde(default)]
    reply_to: Vec<Address>,
}

/// Whether "Worth a look" may be raised for `message`, whatever answer is
/// stored for it (a rule's answer from before the check was made, say): its
/// sender is authenticated by the receiving provider's results, and neither
/// it nor its conversation looks suspicious, fails its sender checks or is
/// impersonated. Fails closed.
async fn may_be_looked_at(tx: &mut Transaction<'_, Postgres>, message: MessageId) -> Result<bool> {
    let row: Option<(String, serde_json::Value, bool)> = sqlx::query_as(
        "SELECT m.sender_address, m.headers,
                (EXISTS (SELECT 1 FROM conversations.decisions d
                          WHERE d.conversation_id = m.conversation_id
                            AND d.question = 'suspicious' AND d.value = 'true'::jsonb)
                 OR EXISTS (SELECT 1 FROM conversations.needs n
                             WHERE n.conversation_id = m.conversation_id
                               AND n.state = 'open' AND n.reason = 'review'))
           FROM conversations.messages m WHERE m.id = $1",
    )
    .bind(message)
    .fetch_optional(&mut **tx)
    .await?;
    let Some((sender, headers, suspicious)) = row else {
        return Ok(false);
    };
    let headers: crate::ingest::Headers = serde_json::from_value(headers).unwrap_or_default();
    let results = headers.authentication_results.as_deref();
    Ok(!suspicious
        && !sender_checks_failed(&sender, results)
        && !dmarc_refused(results)
        && sender_authenticated(&sender, results))
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
    // Whether the newest message was asked whose move it is.
    let mut move_asked = false;
    if let Some(newest) = &newest {
        recent = newest.sent_at >= Utc::now() - settings.window;
        reread = row.is_some_and(|r| r.message_id == newest.id);
        if recent || reread {
            let answers = answers(tx, newest.id).await?;
            money_asked = answers.iter().any(|a| a.question == triage::MONEY);
            move_asked = answers
                .iter()
                .any(|a| a.question == moves::question::NEXT_MOVE);
            // Its people only matter to a wait, which needs a move answer.
            let people = if move_asked {
                people(tx, newest.id).await?
            } else {
                None
            };
            // Authentication is read only for mail a rule highlighted.
            let highlight = answers
                .iter()
                .any(|a| a.basis == NeedBasis::Taught && a.question == WORTH_A_LOOK && a.is_true());
            let look_ok = highlight && may_be_looked_at(tx, newest.id).await?;
            raised = raise_with(newest.id, &answers, people.as_ref(), settings, look_ok);
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
        // Waits switched off: an open wait keeps its course, though its own
        // message no longer raises it.
        Some(row) if reread && raised.is_none() && !settings.waits && row.need()?.is_wait() => {
            false
        }
        Some(row) if reread && raised.is_none() => !matches!(
            row.verdict_on(row.message_id)?,
            Some(NeedVerdict::Confirmed | NeedVerdict::StillToPay)
        ),
        // A newer message in the conversation was read again for whose move
        // it is and waits on nobody ("never mind, I sent it myself"): the
        // wait it followed is over, replaced by what that message says. A
        // newer message from the person waited on ends it as heard back
        // first (`settle`), and one that raises its own need replaces it.
        Some(row) if !reread && recent && move_asked && raised.is_none() => row.need()?.is_wait(),
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
    (id, inbox): (ConversationId, InboxId),
    state: &(bool, bool),
    row: Option<&Row>,
    need: &Need,
) -> Result<(Since, Option<DateTime<Utc>>)> {
    // A message in spam or the trash needs nothing, as one that is gone.
    // The same row says whether it was opened ("Worth a look" clears then).
    let message: Option<(DateTime<Utc>, bool)> = sqlx::query_as(
        "SELECT sent_at, read FROM conversations.messages
          WHERE id = $1 AND NOT in_spam AND NOT in_trash",
    )
    .bind(need.message)
    .fetch_optional(&mut **tx)
    .await?;
    let sent = message.map(|(sent, _)| sent);
    let opened = message.is_some_and(|(_, read)| read);
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
    let rule = if rule_clears(&answers(tx, need.message).await?, need.question) {
        RuleSays::NotNeeded
    } else {
        RuleSays::Nothing
    };
    let (heard_back, sends) = match (&need.awaited, sent) {
        (Some(awaited), Some(sent)) if need.is_wait() => {
            heard_from(tx, inbox, &awaited.key(), sent).await?
        }
        _ => (false, None),
    };
    Ok((
        Since {
            replied,
            archived: state.0,
            trashed: state.1 || sent.is_none(),
            verdict,
            rule,
            sent_at: sent,
            opened,
            heard_back,
            sends,
        },
        sent,
    ))
}

/// Most messages from the person waited on read for one that passes its
/// sender's checks: past these, the wait is not taken for heard back.
const HEARD_SCAN: i64 = 50;

/// How long after a wait's message the inbox's own messages are read for a
/// nudge to the person: what bounds the scan (`messages_outbound_idx`). A
/// message to them sent later than this does not arm the wait again.
const SENDS_SCAN_DAYS: i32 = 60;

/// Whether mail from `address` (lower-cased) reached `inbox` after `after`
/// (spam aside), in any conversation, that the receiving provider did not
/// mark as failing its sender's checks (a forged From hears nothing); and
/// the inbox's messages to it, in To or Cc, in the [`SENDS_SCAN_DAYS`]
/// after.
async fn heard_from(
    tx: &mut Transaction<'_, Postgres>,
    inbox: InboxId,
    address: &str,
    after: DateTime<Utc>,
) -> Result<(bool, Option<Sends>)> {
    // Served by `messages_inbound_sender_idx`; `sender_address` is stored
    // lower-cased. Oldest first, at most [`HEARD_SCAN`].
    let from_them: Vec<serde_json::Value> = sqlx::query_scalar(
        "SELECT headers FROM conversations.messages
          WHERE inbox_id = $1 AND direction = 'inbound' AND sender_address = $2
            AND sent_at > $3 AND NOT in_spam
          ORDER BY sent_at, id LIMIT $4",
    )
    .bind(inbox)
    .bind(address)
    .bind(after)
    .bind(HEARD_SCAN)
    .fetch_all(&mut **tx)
    .await?;
    let heard = from_them
        .iter()
        .any(|headers| !super::person::distrusted("inbound", address, false, headers, false));
    // Served by `messages_outbound_idx` (inbox, sent_at): a bounded range.
    let (last, count): (Option<DateTime<Utc>>, i64) = sqlx::query_as(
        "SELECT max(m.sent_at), count(*) FROM conversations.messages m
          WHERE m.inbox_id = $1 AND m.direction = 'outbound' AND m.sent_at > $3
            AND m.sent_at <= $3 + make_interval(days => $4)
            AND EXISTS (SELECT 1 FROM jsonb_array_elements(
                    CASE WHEN jsonb_typeof(m.recipients->'to') = 'array'
                         THEN m.recipients->'to' ELSE '[]'::jsonb END ||
                    CASE WHEN jsonb_typeof(m.recipients->'cc') = 'array'
                         THEN m.recipients->'cc' ELSE '[]'::jsonb END) r
                         WHERE lower(trim(r->>'address')) = $2)",
    )
    .bind(inbox)
    .bind(address)
    .bind(after)
    .bind(SENDS_SCAN_DAYS)
    .fetch_one(&mut **tx)
    .await?;
    let sends = last.map(|last| Sends {
        last,
        count: i32::try_from(count).unwrap_or(i32::MAX),
    });
    Ok((heard, sends))
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
    // Its dates, as Scout read them from its message's email.
    let read = dates(tx, need.message, need.reason).await?;
    let need = need.dated(read);

    let clock = Clock::now(settings);
    let (since, sent) = since(
        tx,
        (id, conversation.inbox),
        &conversation_state,
        row.as_ref(),
        &need,
    )
    .await?;
    let settled = match settle(need, since, clock) {
        // Its own message no longer raises it: cleared as replaced.
        Settled::Open(need) if withdrawn => Settled::Cleared(need, ClearCause::Superseded),
        settled => settled,
    };
    let wake = next_wake(&settled, sent, clock);

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
        // Unchanged, but when it next changes may be new (a need from
        // before the clock existed): only over a stored row.
        if row.is_some() {
            set_clock(tx, conversation.workspace_id, id, wake).await?;
        }
        return Ok(Pass::Done(Refreshed::Settled));
    }
    let cleared_by = match cause {
        Some(ClearCause::Done | ClearCause::NotNeeded) => row.as_ref().and_then(|r| r.verdict_by),
        _ => None,
    };
    // A runtime query, so the columns added since need no entry in the
    // `.sqlx` offline cache.
    let revision: i64 = sqlx::query_scalar(
        "INSERT INTO conversations.needs
             (conversation_id, workspace_id, message_id, reason, trust, basis, question,
              decision_id, due_on, urgent, state, cleared_because, cleared_by, cleared_at,
              latest_inbound_id, lapses_at, awaited_name, awaited_address, nudge_at, nudges)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13,
                 CASE WHEN $12::text IS NULL THEN NULL ELSE now() END, $14, $15, $16, $17, $18,
                 $19)
         ON CONFLICT (conversation_id) DO UPDATE
            SET message_id = EXCLUDED.message_id, reason = EXCLUDED.reason,
                trust = EXCLUDED.trust, basis = EXCLUDED.basis, question = EXCLUDED.question,
                decision_id = EXCLUDED.decision_id, due_on = EXCLUDED.due_on,
                urgent = EXCLUDED.urgent, state = EXCLUDED.state,
                cleared_because = EXCLUDED.cleared_because, cleared_by = EXCLUDED.cleared_by,
                cleared_at = EXCLUDED.cleared_at, latest_inbound_id = EXCLUDED.latest_inbound_id,
                lapses_at = EXCLUDED.lapses_at, awaited_name = EXCLUDED.awaited_name,
                awaited_address = EXCLUDED.awaited_address, nudge_at = EXCLUDED.nudge_at,
                nudges = EXCLUDED.nudges,
                revision = conversations.needs.revision + 1, updated_at = now()
         RETURNING revision",
    )
    .bind(id)
    .bind(conversation.workspace_id)
    .bind(need.message)
    .bind(need.reason.as_str())
    .bind(need.trust.as_str())
    .bind(need.basis.as_str())
    .bind(need.question.as_str())
    .bind(need.decision)
    .bind(need.due_on)
    .bind(need.urgent)
    .bind(if cause.is_some() { "cleared" } else { "open" })
    .bind(cause.map(ClearCause::as_str))
    .bind(cleared_by.map(|u| *u.as_uuid()))
    .bind(latest)
    .bind(need.lapses_at)
    .bind(need.awaited.as_ref().map(|a| a.name.as_str()))
    .bind(need.awaited.as_ref().map(|a| a.address.as_str()))
    .bind(need.nudge_at)
    .bind(need.nudges)
    .fetch_one(&mut **tx)
    .await?;
    set_clock(tx, conversation.workspace_id, id, wake).await?;
    // A need that opens for a message (new, or back after being cleared)
    // gets Scout's read of the email, through a job: never here, and
    // once per message however often the need is worked out again. Mail
    // that may be phishing gets none: a summary would retell the scam in
    // Scout's voice, and the warning is what matters there.
    let opened = cause.is_none()
        && row
            .as_ref()
            .is_none_or(|r| r.state != "open" || r.message_id != need.message);
    // Only a message the decide pass flagged is read: a plain question gets
    // no read on arrival, and its summary comes when a member opens it.
    if settings.summaries && opened && need.question != NeedQuestion::Suspicious {
        let stored = answers(tx, need.message).await?;
        if super::dated::flag(&stored, &settings.dated).is_some() {
            crate::features::read_message::enqueue(
                tx,
                conversation.workspace_id.into(),
                need.message,
                crate::features::read_message::store::Trigger::Need,
            )
            .await?;
        }
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
                        lapses_at: need.lapses_at,
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
    /// When it is due, as Scout read the email: none until it has, or when
    /// the email names no date.
    pub due_on: Option<NaiveDate>,
    /// Whether the due date has passed.
    pub overdue: bool,
    /// When what it asks stops being possible, as Scout read the email
    /// (never for a bill or a warning).
    pub lapses_at: Option<DateTime<Utc>>,
    /// Whether it is for today.
    pub urgent: bool,
    /// The message it comes from.
    pub message_id: MessageId,
    /// When that message was sent.
    pub sent_at: Option<DateTime<Utc>>,
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
    /// Who a `waiting` or `nudge` need waits on: their name and address as
    /// the email's headers gave them (campbooks-cloud#293). Absent for every
    /// other need.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub waiting_on: Option<Address>,
    /// How many times the member wrote to them since (a `waiting` or
    /// `nudge` need only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nudges: Option<u32>,
}

/// A conversation's ask that lapsed (campbooks-cloud#275): what it was and
/// when it stopped being possible, while its message is still the newest.
/// Clients show it as "Expired".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct LapsedNeed {
    /// Why it was needed.
    pub reason: NeedReason,
    /// The question that raised it.
    pub question: NeedQuestion,
    /// The message it came from.
    pub message_id: MessageId,
    /// When what it asked stopped being possible.
    pub lapsed_at: DateTime<Utc>,
}

/// A row of [`ATTACH`].
#[derive(sqlx::FromRow)]
struct AttachRow {
    conversation_id: ConversationId,
    message_id: MessageId,
    reason: String,
    trust: String,
    basis: String,
    question: String,
    due_on: Option<NaiveDate>,
    lapses_at: Option<DateTime<Utc>>,
    urgent: bool,
    state: String,
    sender: Option<serde_json::Value>,
    sent_at: Option<DateTime<Utc>>,
    reasons: Option<serde_json::Value>,
    awaited_name: Option<String>,
    awaited_address: Option<String>,
    nudges: i32,
}

/// The open needs of some conversations, and their lapsed ones while the
/// lapsed need's message is still the newest inbound message that is not in
/// spam or the trash. A runtime query, so the columns added since need no
/// entry in the `.sqlx` offline cache.
const ATTACH: &str = "SELECT n.conversation_id, n.message_id, n.reason, n.trust, n.basis, \
            n.question, n.due_on, n.lapses_at, n.urgent, n.state, m.sender, m.sent_at, \
            d.reasons, n.awaited_name, n.awaited_address, n.nudges \
       FROM conversations.needs n \
       LEFT JOIN conversations.messages m ON m.id = n.message_id \
       LEFT JOIN conversations.decisions d ON d.id = n.decision_id \
      WHERE n.conversation_id = ANY($1) \
        AND (n.state = 'open' \
             OR (n.cleared_because = 'lapsed' AND n.lapses_at IS NOT NULL \
                 AND m.id IS NOT NULL AND NOT m.in_spam AND NOT m.in_trash \
                 AND NOT EXISTS ( \
                     SELECT 1 FROM conversations.messages later \
                      WHERE later.conversation_id = n.conversation_id \
                        AND later.direction = 'inbound' \
                        AND NOT later.in_spam AND NOT later.in_trash \
                        AND (later.sent_at, later.id) > (m.sent_at, m.id))))";

/// Fill in each summary's open [`NeedView`], or its [`LapsedNeed`] when its
/// ask lapsed (an open need whose lapse time passed counts as lapsed before
/// the clock has woken it).
pub async fn attach(conn: &mut PgConnection, summaries: &mut [ConversationSummary]) -> Result<()> {
    let ids: Vec<ConversationId> = summaries.iter().map(|s| s.id).collect();
    if ids.is_empty() {
        return Ok(());
    }
    let rows: Vec<AttachRow> = sqlx::query_as(ATTACH)
        .bind(ids.as_slice())
        .fetch_all(&mut *conn)
        .await?;
    let now = Utc::now();
    let today = now.date_naive();
    for row in rows {
        let Some(summary) = summaries.iter_mut().find(|s| s.id == row.conversation_id) else {
            continue;
        };
        let reason: NeedReason = row.reason.parse()?;
        let question: NeedQuestion = row.question.parse()?;
        let lapsed = row.state != "open" || row.lapses_at.is_some_and(|at| at <= now);
        if lapsed {
            if let Some(lapsed_at) = row.lapses_at {
                summary.lapsed_need = Some(LapsedNeed {
                    reason,
                    question,
                    message_id: row.message_id,
                    lapsed_at,
                });
            }
            continue;
        }
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
            reason,
            trust: row.trust.parse()?,
            basis: row.basis.parse()?,
            question,
            rule: reasons.first().map(|r| r.rule.clone()),
            due_on: row.due_on,
            overdue: row.due_on.is_some_and(|due| due < today),
            lapses_at: row.lapses_at,
            urgent: row.urgent,
            message_id: row.message_id,
            sent_at: row.sent_at,
            quote,
            actions: None,
            waiting_on: row
                .awaited_address
                .filter(|_| matches!(reason, NeedReason::Waiting | NeedReason::Nudge))
                .map(|address| Address {
                    name: row.awaited_name.filter(|name| !name.trim().is_empty()),
                    address,
                }),
            nudges: matches!(reason, NeedReason::Waiting | NeedReason::Nudge)
                .then(|| u32::try_from(row.nudges).unwrap_or_default()),
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
            options: serde_json::Value::Null,
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
        raise(MessageId::new(), answers, &settings(), true)
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
    fn a_stored_highlight_raises_nothing_on_mail_that_may_not_be_looked_at() {
        let look = answer(WORTH_A_LOOK, NeedBasis::Taught, json!(true), true);
        assert!(raise(MessageId::new(), &[look], &settings(), false).is_none());
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
            cause(&settle(need(NeedReason::WorthALook), opened, clock())),
            Some(ClearCause::Opened)
        );
        // Opening settles nothing else: a bill stays until answered.
        assert_eq!(cause(&settle(need(NeedReason::Pay), opened, clock())), None);
        assert_eq!(
            cause(&settle(
                need(NeedReason::WorthALook),
                Since::default(),
                clock()
            )),
            None
        );
        // Seen (Done) clears it too.
        let seen = Since {
            verdict: Some(NeedVerdict::Done),
            ..Since::default()
        };
        assert_eq!(
            cause(&settle(need(NeedReason::WorthALook), seen, clock())),
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
            cause(&settle(need(NeedReason::Pay), by_rule, clock())),
            Some(ClearCause::Rule)
        );
        let mut confirmed = need(NeedReason::Reply);
        confirmed.basis = NeedBasis::Member;
        assert_eq!(cause(&settle(confirmed, by_rule, clock())), None);
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
            lapses_at: None,
            urgent: false,
            awaited: None,
            nudge_at: None,
            nudges: 0,
        }
    }

    /// Now, with the shipped 14-day window and 3-day wait.
    fn clock() -> Clock {
        Clock {
            now: Utc::now(),
            window: TimeDelta::days(14),
            wait_days: 3,
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
                    cause(&settle(need(reason), since, clock())),
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
            cause(&settle(need(NeedReason::Pay), done, clock())),
            Some(ClearCause::Done)
        );
        let confirmed = Since {
            verdict: Some(NeedVerdict::Confirmed),
            ..Since::default()
        };
        let mut question = need(NeedReason::Review);
        question.trust = NeedTrust::Check;
        match settle(question, confirmed, clock()) {
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
            cause(&settle(question.clone(), paid, clock())),
            Some(ClearCause::Done)
        );
        let to_pay = Since {
            verdict: Some(NeedVerdict::StillToPay),
            ..Since::default()
        };
        match settle(question, to_pay, clock()) {
            Settled::Open(need) => assert_eq!(
                (need.reason, need.trust, need.basis),
                (NeedReason::Pay, NeedTrust::Sure, NeedBasis::Member)
            ),
            Settled::Cleared(..) => panic!("still to pay stays open"),
        }
    }

    fn sent(days_ago: i64) -> Since {
        Since {
            sent_at: Some(Utc::now() - TimeDelta::days(days_ago)),
            ..Since::default()
        }
    }

    fn lapsing(mut need: Need, minutes: i64) -> Need {
        need.lapses_at = Some(Utc::now() + TimeDelta::minutes(minutes));
        need
    }

    fn model(reason: NeedReason, question: NeedQuestion) -> Need {
        let mut need = need(reason);
        need.basis = NeedBasis::Model;
        need.question = question;
        need
    }

    /// The bug of campbooks-cloud#275: a sign-in link valid for 10 minutes,
    /// long used, still "asked you to do something".
    #[test]
    fn a_lapsed_ask_is_cleared() {
        let link = lapsing(model(NeedReason::Do, NeedQuestion::AsksAction), -1);
        assert_eq!(
            cause(&settle(link.clone(), sent(0), clock())),
            Some(ClearCause::Lapsed)
        );
        // Still possible: open, as it was.
        let fresh = lapsing(model(NeedReason::Do, NeedQuestion::AsksAction), 9);
        assert_eq!(cause(&settle(fresh, sent(0), clock())), None);
        // A member's own ask lapses too: nothing is left to do.
        let mut confirmed = link.clone();
        confirmed.basis = NeedBasis::Member;
        assert_eq!(
            cause(&settle(confirmed, sent(0), clock())),
            Some(ClearCause::Lapsed)
        );
        // So does a question, and a reply.
        let mut question = link;
        question.reason = NeedReason::Review;
        question.trust = NeedTrust::Check;
        assert_eq!(
            cause(&settle(question, sent(0), clock())),
            Some(ClearCause::Lapsed)
        );
        let reply = lapsing(model(NeedReason::Reply, NeedQuestion::NeedsReply), -5);
        assert_eq!(
            cause(&settle(reply, sent(0), clock())),
            Some(ClearCause::Lapsed)
        );
    }

    #[test]
    fn a_bill_never_lapses_and_a_warning_keeps_its_rules() {
        let mut bill = lapsing(model(NeedReason::Pay, NeedQuestion::Money), -60);
        bill.trust = NeedTrust::Check;
        assert_eq!(cause(&settle(bill.clone(), sent(30), clock())), None);
        // Its dates: a due date, never a lapse time.
        let dated = bill.dated(Dates {
            due_on: Some(Utc::now().date_naive()),
            lapses_at: Some(Utc::now()),
        });
        assert_eq!(dated.lapses_at, None);
        assert!(dated.due_on.is_some());
        let warning = lapsing(model(NeedReason::Review, NeedQuestion::Suspicious), -60);
        match settle(warning, sent(30), clock()) {
            Settled::Open(need) => assert_eq!(
                (need.reason, need.trust, need.question),
                (
                    NeedReason::Review,
                    NeedTrust::Sure,
                    NeedQuestion::Suspicious
                )
            ),
            Settled::Cleared(..) => panic!("a warning neither lapses nor goes stale"),
        }
        assert_eq!(
            model(NeedReason::Review, NeedQuestion::Suspicious)
                .dated(Dates {
                    due_on: None,
                    lapses_at: Some(Utc::now()),
                })
                .lapses_at,
            None
        );
    }

    /// A taught "Worth a look" rule keeps its need until the message is
    /// opened: no lapse time read from the mail undoes it.
    #[test]
    fn worth_a_look_never_lapses() {
        let mut look = lapsing(model(NeedReason::WorthALook, NeedQuestion::WorthALook), -60);
        look.basis = NeedBasis::Taught;
        assert!(!look.can_lapse());
        match settle(look.clone(), sent(30), clock()) {
            Settled::Open(need) => assert_eq!(
                (need.reason, need.trust, need.question),
                (
                    NeedReason::WorthALook,
                    NeedTrust::Sure,
                    NeedQuestion::WorthALook
                )
            ),
            Settled::Cleared(_, cause) => panic!("worth a look stays until opened, not {cause:?}"),
        }
        // Its dates carry no lapse time, and the clock never wakes it.
        assert_eq!(
            look.clone()
                .dated(Dates {
                    due_on: None,
                    lapses_at: Some(Utc::now() + TimeDelta::minutes(5)),
                })
                .lapses_at,
            None
        );
        assert_eq!(
            next_wake(
                &Settled::Open(look),
                Some(Utc::now() - TimeDelta::days(30)),
                clock()
            ),
            None
        );
    }

    #[test]
    fn an_old_undated_ask_becomes_a_question() {
        for (reason, raised, asked) in [
            (
                NeedReason::Do,
                NeedQuestion::AsksAction,
                NeedQuestion::StillToDo,
            ),
            (
                NeedReason::Reply,
                NeedQuestion::NeedsReply,
                NeedQuestion::StillNeedsReply,
            ),
        ] {
            match settle(model(reason, raised), sent(21), clock()) {
                Settled::Open(need) => assert_eq!(
                    (need.reason, need.trust, need.question),
                    (NeedReason::Review, NeedTrust::Check, asked),
                    "{reason:?}"
                ),
                Settled::Cleared(..) => panic!("an old ask stays, as a question"),
            }
            // Inside the window: as it was.
            match settle(model(reason, raised), sent(10), clock()) {
                Settled::Open(need) => assert_eq!(need.reason, reason),
                Settled::Cleared(..) => panic!("a recent ask stays"),
            }
        }
    }

    #[test]
    fn a_dated_or_confirmed_ask_is_not_asked_again() {
        let mut dated = model(NeedReason::Do, NeedQuestion::AsksAction);
        dated.due_on = Some(Utc::now().date_naive() - TimeDelta::days(3));
        match settle(dated, sent(21), clock()) {
            Settled::Open(need) => assert_eq!(need.reason, NeedReason::Do),
            Settled::Cleared(..) => panic!("a dated ask stays"),
        }
        let ahead = lapsing(model(NeedReason::Do, NeedQuestion::AsksAction), 60);
        match settle(ahead, sent(21), clock()) {
            Settled::Open(need) => assert_eq!(need.reason, NeedReason::Do),
            Settled::Cleared(..) => panic!("an ask still possible stays"),
        }
        let mut mine = model(NeedReason::Do, NeedQuestion::AsksAction);
        mine.basis = NeedBasis::Member;
        match settle(mine, sent(60), clock()) {
            Settled::Open(need) => {
                assert_eq!((need.reason, need.trust), (NeedReason::Do, NeedTrust::Sure));
            }
            Settled::Cleared(..) => panic!("a member's ask stays"),
        }
    }

    #[test]
    fn yes_to_still_needed_keeps_the_ask_for_good() {
        // Raised again from its answers, with the member's Yes on it.
        let yes = Since {
            verdict: Some(NeedVerdict::Confirmed),
            ..sent(40)
        };
        match settle(
            model(NeedReason::Do, NeedQuestion::AsksAction),
            yes,
            clock(),
        ) {
            Settled::Open(need) => assert_eq!(
                (need.reason, need.trust, need.basis, need.question),
                (
                    NeedReason::Do,
                    NeedTrust::Sure,
                    NeedBasis::Member,
                    NeedQuestion::AsksAction
                )
            ),
            Settled::Cleared(..) => panic!("a confirmed ask stays"),
        }
        // The stored question itself, confirmed.
        let mut question = model(NeedReason::Review, NeedQuestion::StillNeedsReply);
        question.trust = NeedTrust::Check;
        match settle(question, yes, clock()) {
            Settled::Open(need) => assert_eq!(
                (need.reason, need.question, need.basis),
                (
                    NeedReason::Reply,
                    NeedQuestion::NeedsReply,
                    NeedBasis::Member
                )
            ),
            Settled::Cleared(..) => panic!("a confirmed question stays"),
        }
        let no = Since {
            verdict: Some(NeedVerdict::NotNeeded),
            ..sent(40)
        };
        assert_eq!(
            cause(&settle(
                model(NeedReason::Do, NeedQuestion::AsksAction),
                no,
                clock()
            )),
            Some(ClearCause::NotNeeded)
        );
    }

    #[test]
    fn a_rule_reads_a_still_needed_question_as_its_ask() {
        let taught_no = [answer(
            triage::ASKS_ACTION,
            NeedBasis::Taught,
            json!(false),
            true,
        )];
        assert!(rule_clears(&taught_no, NeedQuestion::StillToDo));
        assert!(!rule_clears(&taught_no, NeedQuestion::StillNeedsReply));
    }

    #[test]
    fn the_next_wake_is_the_first_change_still_ahead() {
        let clock = clock();
        let sent_at = clock.now - TimeDelta::days(10);
        let ask = model(NeedReason::Do, NeedQuestion::AsksAction);
        // Undated: when its message leaves the window.
        assert_eq!(
            next_wake(&Settled::Open(ask.clone()), Some(sent_at), clock),
            Some(sent_at + TimeDelta::days(14))
        );
        // Lapsing in 10 minutes: then.
        let mut link = ask.clone();
        link.lapses_at = Some(clock.now + TimeDelta::minutes(10));
        assert_eq!(
            next_wake(&Settled::Open(link.clone()), Some(sent_at), clock),
            link.lapses_at
        );
        // A bill, a member's undated ask, a cleared need: nothing to wait for.
        let mut bill = model(NeedReason::Pay, NeedQuestion::Money);
        bill.lapses_at = link.lapses_at;
        assert_eq!(next_wake(&Settled::Open(bill), Some(sent_at), clock), None);
        let mut mine = ask.clone();
        mine.basis = NeedBasis::Member;
        assert_eq!(next_wake(&Settled::Open(mine), Some(sent_at), clock), None);
        assert_eq!(
            next_wake(
                &Settled::Cleared(link, ClearCause::Done),
                Some(sent_at),
                clock
            ),
            None
        );
        // Already past the window: settled as a question, nothing ahead.
        assert_eq!(
            next_wake(
                &Settled::Open(ask),
                Some(clock.now - TimeDelta::days(20)),
                clock
            ),
            None
        );
    }

    // --- Waiting on someone (campbooks-cloud#293) ---

    fn at(rfc3339: &str) -> DateTime<Utc> {
        rfc3339.parse().unwrap()
    }

    fn day(ymd: &str) -> NaiveDate {
        ymd.parse().unwrap()
    }

    #[test]
    fn a_nudge_comes_working_days_later_at_the_same_time_of_day() {
        // Wednesday 14 October 10:00, three working days: Monday 10:00.
        assert_eq!(
            nudge_time(at("2026-10-14T10:00:00Z"), None, 3),
            at("2026-10-19T10:00:00Z")
        );
        // A date for them, Friday 16 October: the working day after it, at
        // the time the message was sent.
        assert_eq!(
            nudge_time(at("2026-10-14T08:30:00Z"), Some(day("2026-10-16")), 3),
            at("2026-10-19T08:30:00Z")
        );
        // Nudged on Monday 19 October 11:00: Thursday 11:00.
        assert_eq!(
            after_working_days(at("2026-10-19T11:00:00Z"), 3),
            at("2026-10-22T11:00:00Z")
        );
        // Sent on a Saturday: Monday is the first working day.
        assert_eq!(
            nudge_time(at("2026-10-17T15:00:00Z"), None, 1),
            at("2026-10-19T15:00:00Z")
        );
        assert_eq!(
            nudge_time(at("2026-10-17T15:00:00Z"), None, 3),
            at("2026-10-21T15:00:00Z")
        );
        // A date on a weekend: the Monday after.
        assert_eq!(
            nudge_time(at("2026-10-14T10:00:00Z"), Some(day("2026-10-17")), 3),
            at("2026-10-19T10:00:00Z")
        );
        // Fifteen working days: three weeks.
        assert_eq!(
            after_working_days(at("2026-10-14T10:00:00Z"), 15),
            at("2026-11-04T10:00:00Z")
        );
    }

    fn party(name: &str, address: &str) -> Party {
        Party {
            name: name.into(),
            address: address.into(),
        }
    }

    /// Joao writes to Ana Costa, copying the inbox.
    fn joao_to_ana() -> Envelope {
        Envelope {
            from: party("Joao", "joao@studio.example"),
            to: vec![party("Ana Costa", "ana@studio.example")],
            cc: vec![party("Sam", "sam@example.com")],
            recipient: Some(Recipient {
                name: "Sam".into(),
                addresses: vec!["sam@example.com".into()],
                shared: false,
            }),
            ..Envelope::default()
        }
    }

    fn next_move(label: &str, confidence: f64, acted: bool) -> Answer {
        confident(
            answer(
                moves::question::NEXT_MOVE,
                NeedBasis::Model,
                json!(label),
                acted,
            ),
            confidence,
        )
    }

    /// The model's `awaited` answer `label`, stored with the options the
    /// decide pass asked about `people`.
    fn awaited(people: &Envelope, label: &str) -> Answer {
        let question = moves::awaited_question(people).unwrap();
        let options = match &question.kind {
            campbooks_ai::decide::QuestionKind::Choice { options, .. } => options
                .iter()
                .map(|o| json!({"label": o.label, "description": o.description}))
                .collect::<Vec<_>>(),
            other => panic!("not a choice: {other:?}"),
        };
        let mut a = answer(
            moves::question::AWAITED,
            NeedBasis::Model,
            json!(label),
            false,
        );
        a.options = json!(options);
        a
    }

    fn quiet() -> Vec<Answer> {
        vec![
            stream(NeedBasis::Model, "primary", true),
            answer(triage::NEEDS_REPLY, NeedBasis::Model, json!(false), true),
            answer(triage::ASKS_ACTION, NeedBasis::Model, json!(false), true),
        ]
    }

    fn waits(answers: &[Answer], people: &Envelope) -> Option<Need> {
        raise_with(MessageId::new(), answers, Some(people), &settings(), true)
    }

    #[test]
    fn ana_was_asked_to_write() {
        let people = joao_to_ana();
        let mut answers = quiet();
        answers.push(next_move("someone_else", 0.91, false));
        answers.push(awaited(&people, "p1"));
        let need = waits(&answers, &people).expect("a wait on Ana");
        assert_eq!(
            (need.reason, need.trust, need.basis, need.question),
            (
                NeedReason::Waiting,
                NeedTrust::Check,
                NeedBasis::Model,
                NeedQuestion::NextMove
            )
        );
        assert_eq!(
            need.awaited,
            Some(Awaited {
                name: "Ana Costa".into(),
                address: "ana@studio.example".into()
            })
        );
        assert!(!need.can_lapse());
        // Without the people (a pass that does not read them), no wait.
        assert_eq!(raised(&answers), None);
    }

    #[test]
    fn a_promise_from_the_sender_waits_on_the_sender() {
        let people = joao_to_ana();
        let mut answers = quiet();
        answers.push(next_move("sender", 0.86, false));
        let need = waits(&answers, &people).unwrap();
        assert_eq!(
            need.awaited.map(|a| a.address),
            Some("joao@studio.example".to_owned())
        );
        // A sender that is a machine is never waited on.
        let mut machine = joao_to_ana();
        machine.from = party("Tickets", "no-reply@tickets.example.com");
        assert_eq!(waits(&answers, &machine), None);
    }

    #[test]
    fn never_a_wait_on_the_inbox_itself() {
        // Mail the inbox sent itself, filed as received: its sender is the
        // inbox, whatever the case or spaces.
        let mut own = joao_to_ana();
        own.from = party("Sam", " SAM@example.com ");
        let mut answers = quiet();
        answers.push(next_move("sender", 0.95, true));
        assert_eq!(waits(&answers, &own), None);
    }

    #[test]
    fn no_new_wait_while_waits_are_switched_off() {
        let people = joao_to_ana();
        let mut answers = quiet();
        answers.push(next_move("someone_else", 0.91, false));
        answers.push(awaited(&people, "p1"));
        assert!(waits(&answers, &people).is_some());
        let off = settings().with_waits(false);
        assert_eq!(
            raise_with(MessageId::new(), &answers, Some(&people), &off, true),
            None
        );
    }

    #[test]
    fn unsure_or_nobody_named_waits_on_nobody() {
        let people = joao_to_ana();
        let mut unsure = quiet();
        unsure.push(next_move("someone_else", 0.6, false));
        unsure.push(awaited(&people, "p1"));
        assert_eq!(waits(&unsure, &people), None);
        let mut nobody = quiet();
        nobody.push(next_move("someone_else", 0.9, false));
        nobody.push(awaited(&people, "none"));
        assert_eq!(waits(&nobody, &people), None);
        // No `awaited` answer at all.
        let mut missing = quiet();
        missing.push(next_move("someone_else", 0.9, false));
        assert_eq!(waits(&missing, &people), None);
        // The recipient's move, or nobody's: no wait.
        for label in ["recipient", "nobody"] {
            let mut answers = quiet();
            answers.push(next_move(label, 0.99, false));
            assert_eq!(waits(&answers, &people), None, "{label}");
        }
    }

    #[test]
    fn a_stored_label_is_only_read_as_the_person_it_named() {
        let people = joao_to_ana();
        let mut answers = quiet();
        answers.push(next_move("someone_else", 0.9, false));
        let asked = awaited(&people, "p1");
        answers.push(asked.clone());
        // The headers now put someone else first: `p1` is not Ana any more.
        let mut changed = people.clone();
        changed.to.insert(0, party("Rui", "rui@studio.example"));
        assert_eq!(waits(&answers, &changed), None);
        // Options stored without their descriptions say nothing: no wait.
        let mut bare = asked;
        bare.options = json!(["p1", "none"]);
        answers.pop();
        answers.push(bare);
        assert_eq!(waits(&answers, &people), None);
    }

    #[test]
    fn the_members_move_and_machine_mail_win() {
        let people = joao_to_ana();
        let wait = [
            next_move("someone_else", 0.95, false),
            awaited(&people, "p1"),
        ];
        // The message also raises a reply: the reply, nothing waits.
        let mut reply = vec![
            stream(NeedBasis::Model, "primary", true),
            answer(triage::NEEDS_REPLY, NeedBasis::Model, json!(true), true),
        ];
        reply.extend(wait.clone());
        assert_eq!(waits(&reply, &people).unwrap().reason, NeedReason::Reply);
        // List mail, sorted out of Primary, or a no-reply sender: no wait.
        let mut list = quiet();
        list.push(with_reason(
            answer(triage::NEEDS_REPLY, NeedBasis::Rules, json!(false), true),
            facts::LIST_MAIL,
            "List-Id",
        ));
        list.extend(wait.clone());
        assert_eq!(waits(&list, &people), None);
        let mut updates = vec![stream(NeedBasis::Model, "updates", true)];
        updates.extend(wait.clone());
        assert_eq!(waits(&updates, &people), None);
        // No stream answer accepted: a wait.
        assert!(waits(&wait, &people).is_some());
        // A rule the member taught saying "never": nothing.
        let mut never = wait.to_vec();
        never.push(answer(SURFACE, NeedBasis::Taught, json!("never"), true));
        assert_eq!(waits(&never, &people), None);
    }

    #[test]
    fn once_measured_an_accepted_move_takes_away_the_models_reply() {
        let people = joao_to_ana();
        // The policy accepts `next_move` (acted) and the model's reply.
        let answers = [
            stream(NeedBasis::Model, "primary", true),
            answer(triage::NEEDS_REPLY, NeedBasis::Model, json!(true), true),
            answer(triage::ASKS_ACTION, NeedBasis::Model, json!(true), true),
            next_move("someone_else", 0.95, true),
            awaited(&people, "p1"),
        ];
        let need = waits(&answers, &people).unwrap();
        assert_eq!(
            (need.reason, need.trust),
            (NeedReason::Waiting, NeedTrust::Sure)
        );
        // Not measured (not acted): the reply is raised as before.
        let mut unmeasured = answers.to_vec();
        unmeasured[3].acted = false;
        assert_eq!(
            waits(&unmeasured, &people).unwrap().reason,
            NeedReason::Reply
        );
        // Accepted "nobody": no reply, no wait either.
        let mut nobody = answers.to_vec();
        nobody[3] = next_move("nobody", 0.95, true);
        assert_eq!(waits(&nobody, &people), None);
        // A taught rule's ask still stands.
        let mut taught = answers.to_vec();
        taught.push(answer(
            triage::NEEDS_REPLY,
            NeedBasis::Taught,
            json!(true),
            true,
        ));
        assert_eq!(waits(&taught, &people).unwrap().reason, NeedReason::Reply);
        // The shipped policy never accepts it.
        assert_eq!(
            Policy::baseline().switch(moves::question::NEXT_MOVE, Source::Model),
            super::super::policy::Switch::Record
        );
    }

    fn wait_on_ana(sent: DateTime<Utc>) -> (Need, Since) {
        let mut need = model(NeedReason::Waiting, NeedQuestion::NextMove);
        need.trust = NeedTrust::Check;
        need.awaited = Some(Awaited {
            name: "Ana Costa".into(),
            address: "ana@studio.example".into(),
        });
        let since = Since {
            sent_at: Some(sent),
            ..Since::default()
        };
        (need, since)
    }

    fn open(settled: Settled) -> Need {
        match settled {
            Settled::Open(need) => need,
            Settled::Cleared(need, cause) => panic!("cleared as {cause:?}: {need:?}"),
        }
    }

    #[test]
    fn a_wait_becomes_a_nudge_at_its_time_and_is_never_dropped() {
        let clock = clock();
        // Sent an hour ago: waiting, nudged three working days later.
        let sent = clock.now - TimeDelta::hours(1);
        let (need, since) = wait_on_ana(sent);
        let waiting = open(settle(need.clone(), since, clock));
        assert_eq!(waiting.reason, NeedReason::Waiting);
        assert_eq!(waiting.nudge_at, Some(after_working_days(sent, 3)));
        assert_eq!(waiting.due_on, waiting.nudge_at.map(|at| at.date_naive()));
        assert_eq!(
            next_wake(&Settled::Open(waiting.clone()), Some(sent), clock),
            waiting.nudge_at
        );
        // Long ago: a Nudge, however old, and nothing more to wake for.
        let old = clock.now - TimeDelta::days(40);
        let (need, since) = wait_on_ana(old);
        let nudge = open(settle(need, since, clock));
        assert_eq!(nudge.reason, NeedReason::Nudge);
        assert_eq!(next_wake(&Settled::Open(nudge), Some(old), clock), None);
    }

    #[test]
    fn archiving_leaves_a_wait_and_hearing_back_ends_it() {
        let clock = clock();
        let (wait, since) = wait_on_ana(clock.now - TimeDelta::days(1));
        let archived = Since {
            archived: true,
            replied: true,
            ..since
        };
        assert_eq!(
            open(settle(wait.clone(), archived, clock)).reason,
            NeedReason::Waiting
        );
        let heard = Since {
            heard_back: true,
            ..since
        };
        assert_eq!(
            cause(&settle(wait.clone(), heard, clock)),
            Some(ClearCause::HeardBack)
        );
        // Heard back says nothing to anything but a wait.
        assert_eq!(cause(&settle(need(NeedReason::Reply), heard, clock)), None);
        // Trash wins; Heard from them is Done; Not waiting is Not needed.
        let trashed = Since {
            trashed: true,
            ..since
        };
        assert_eq!(
            cause(&settle(wait.clone(), trashed, clock)),
            Some(ClearCause::Trashed)
        );
        for (verdict, cleared) in [
            (NeedVerdict::Done, ClearCause::Done),
            (NeedVerdict::NotNeeded, ClearCause::NotNeeded),
        ] {
            let judged = Since {
                verdict: Some(verdict),
                ..since
            };
            assert_eq!(cause(&settle(wait.clone(), judged, clock)), Some(cleared));
        }
        // Yes, still waiting: a wait Campbooks stands behind.
        let confirmed = Since {
            verdict: Some(NeedVerdict::Confirmed),
            ..since
        };
        let kept = open(settle(wait, confirmed, clock));
        assert_eq!(
            (kept.reason, kept.trust, kept.basis),
            (NeedReason::Waiting, NeedTrust::Sure, NeedBasis::Member)
        );
    }

    #[test]
    fn writing_to_them_arms_the_wait_again() {
        let clock = clock();
        // Nudge time long past, then the member wrote to Ana an hour ago.
        let (wait, since) = wait_on_ana(clock.now - TimeDelta::days(20));
        let last = clock.now - TimeDelta::hours(1);
        let nudged = Since {
            sends: Some(Sends { last, count: 1 }),
            ..since
        };
        let again = open(settle(wait.clone(), nudged, clock));
        assert_eq!(again.reason, NeedReason::Waiting);
        assert_eq!(again.nudges, 1);
        assert_eq!(again.nudge_at, Some(after_working_days(last, 3)));
        // A date read for Ana counts only until the member writes.
        let mut dated = wait;
        dated.due_on = Some(clock.now.date_naive() + TimeDelta::days(30));
        let armed = dated.clone().armed(clock.now, None, 3);
        assert_eq!(armed.nudge_at, Some(nudge_time(clock.now, dated.due_on, 3)));
        let rearmed = dated.armed(clock.now, Some(Sends { last, count: 2 }), 3);
        assert_eq!(rearmed.nudge_at, Some(after_working_days(last, 3)));
        assert_eq!(rearmed.nudges, 2);
        // Anything but a wait is not armed.
        let reply = need(NeedReason::Reply).armed(clock.now, None, 3);
        assert_eq!((reply.nudge_at, reply.due_on), (None, None));
    }
}
