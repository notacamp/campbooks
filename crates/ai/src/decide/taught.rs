//! Taught rules: deterministic condition → action pairs a member taught
//! Campbooks (campbooks-cloud#125).
//!
//! A [`TaughtRule`] holds conditions that must all hold and actions, at most
//! one per [`Slot`] (labels add up). [`evaluate`] reads a message's
//! [`Facts`] and returns, per slot, what the most specific matching rule says
//! ([`Outcome`]). It calls no model and keeps no state: the same facts and the
//! same rules always give the same outcome, so a member can read a rule as a
//! sentence and trust it.
//!
//! Precedence per slot: the highest [`TaughtRule::specificity`], then the
//! latest `confirmed_at`, then the smallest id. Two guards follow: mail that
//! looks suspicious is never hidden (no archive, no read, no move out of
//! Primary, no "needs nothing", no `surface: never`, no "paid"), and a
//! winning `surface: always` keeps every other rule from hiding the mail,
//! the trash and spam included. A rule that puts mail in the trash or spam
//! is the one exception to the first guard: it is about the suspicious mail
//! itself ("Never see emails like this again", campbooks-cloud#169), so it
//! acts on it.
//!
//! [`TaughtRules`] is also a [`Strategy`] for the stream and needs questions,
//! so the eval harness and a [`Cascade`](super::strategy::Cascade) can put it
//! first.

use std::{cmp::Ordering, collections::BTreeSet, time::Instant};

use serde::{Deserialize, Serialize};

use super::{
    Question, QuestionKind,
    signals::email::{Envelope, is_public_suffix, registrable_domain, subject_words, triage},
    strategy::{BoxFuture, Decided, Decisions, Reason, Strategy, local_answer, peaked},
};
use crate::Error;

/// Most conditions a rule may have.
pub const MAX_CONDITIONS: usize = 4;
/// Most actions a rule may have.
pub const MAX_ACTIONS: usize = 4;

/// The stored headers a condition may test.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HeaderName {
    /// `List-Id`.
    ListId,
    /// `List-Unsubscribe`.
    ListUnsubscribe,
    /// `Precedence`.
    Precedence,
    /// `Auto-Submitted`.
    AutoSubmitted,
}

/// A header test: present, or equal to a value (case-insensitive).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct HeaderTest {
    /// Which header.
    pub name: HeaderName,
    /// The value it must have; absent means "present".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
}

/// One test on a message. All of a rule's conditions must hold.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "field", content = "value", rename_all = "snake_case")]
pub enum Condition {
    /// The sender's address is this one (case-insensitive).
    SenderAddress(String),
    /// The sender's registrable domain is this one, or a subdomain of it.
    SenderDomain(String),
    /// The `List-Id` is this one (angle brackets and case ignored).
    ListId(String),
    /// The subject's template ([`subject_template`]) is this one.
    SubjectTemplate(String),
    /// The subject holds these words, in this order, as whole words (case
    /// and accents ignored).
    SubjectContains(String),
    /// A stored header is present, or has a value.
    Header(HeaderTest),
    /// The conversation has this label (its id).
    Label(String),
    /// The conversation is in this stream.
    Stream(String),
    /// The receiving provider's authentication results say the sender's
    /// domain failed its checks (a structured fact, never the content).
    SenderChecksFail,
}

impl Condition {
    /// How narrowly it picks mail: a template, list or header 3; subject
    /// words or an address 2; a domain, label, stream or failed sender
    /// checks 1.
    pub fn weight(&self) -> u32 {
        match self {
            Self::SubjectTemplate(_) | Self::ListId(_) | Self::Header(_) => 3,
            Self::SubjectContains(_) | Self::SenderAddress(_) => 2,
            Self::SenderDomain(_) | Self::Label(_) | Self::Stream(_) | Self::SenderChecksFail => 1,
        }
    }

    fn holds(&self, facts: &Facts<'_>) -> bool {
        match self {
            Self::SenderAddress(address) => facts.sender_address.eq_ignore_ascii_case(address),
            Self::SenderDomain(domain) => {
                // Fail closed: a domain that is (now) a public suffix, like
                // `github.io`, names every site under it and matches nothing.
                if !is_owned_domain(domain) {
                    return false;
                }
                let domain = domain.to_ascii_lowercase();
                let host = facts
                    .sender_address
                    .rsplit_once('@')
                    .map_or("", |(_, d)| d)
                    .to_ascii_lowercase();
                host == domain || host.ends_with(&format!(".{domain}"))
            }
            Self::ListId(list) => facts
                .list_id
                .is_some_and(|l| normalise_list_id(l) == normalise_list_id(list)),
            Self::SubjectTemplate(template) => facts.subject_template == Some(template.as_str()),
            Self::SubjectContains(words) => contains_words(facts.subject, words),
            Self::Header(test) => {
                let value = match test.name {
                    HeaderName::ListId => facts.list_id,
                    HeaderName::ListUnsubscribe => facts.list_unsubscribe,
                    HeaderName::Precedence => facts.precedence,
                    HeaderName::AutoSubmitted => facts.auto_submitted,
                };
                match (&test.value, value) {
                    (_, None) => false,
                    (None, Some(_)) => true,
                    (Some(want), Some(have)) => have.trim().eq_ignore_ascii_case(want.trim()),
                }
            }
            Self::Label(label) => facts.labels.iter().any(|l| l == label),
            Self::Stream(stream) => facts.stream == stream,
            Self::SenderChecksFail => facts.impersonated && !facts.correspondent,
        }
    }
}

/// `always`: never hide this mail; `never`: it never needs anyone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Surface {
    /// Keep it visible: no rule archives it, marks it read or moves it out
    /// of Primary.
    Always,
    /// Raise no need for it.
    Never,
}

/// What a rule does. At most one per [`Slot`]; labels add up.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum Action {
    /// Put the conversation in this stream.
    MoveToStream {
        /// The stream.
        stream: String,
    },
    /// Take it out of the inbox.
    Archive,
    /// Put this label on it (the label's id).
    AddLabel {
        /// The label.
        label: String,
    },
    /// Mark it read.
    MarkRead,
    /// The answer to "needs a reply".
    NeedsReply {
        /// True or false.
        value: bool,
    },
    /// The answer to "asks for an action".
    AsksAction {
        /// True or false.
        value: bool,
    },
    /// Always or never show it.
    Surface {
        /// Which.
        value: Surface,
    },
    /// Put it in the trash.
    Trash,
    /// Report it as spam.
    Spam,
    /// Its bills are paid: Campbooks never asks "Is it paid?" about it.
    MarkPaid,
}

/// Where a rule puts mail it takes out of the way for good.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Bin {
    /// The trash.
    Trash,
    /// Spam (reported).
    Spam,
}

/// The places an action fills; one rule fills each at most once.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Slot {
    /// The stream.
    Stream,
    /// Archived.
    Archive,
    /// Read.
    Read,
    /// A label (additive).
    Label,
    /// "Needs a reply".
    NeedsReply,
    /// "Asks for an action".
    AsksAction,
    /// Always or never show.
    Surface,
    /// The trash or spam.
    Bin,
    /// Whether its bills are paid.
    Money,
}

impl Action {
    /// The slot it fills.
    pub fn slot(&self) -> Slot {
        match self {
            Self::MoveToStream { .. } => Slot::Stream,
            Self::Archive => Slot::Archive,
            Self::AddLabel { .. } => Slot::Label,
            Self::MarkRead => Slot::Read,
            Self::NeedsReply { .. } => Slot::NeedsReply,
            Self::AsksAction { .. } => Slot::AsksAction,
            Self::Surface { .. } => Slot::Surface,
            Self::Trash | Self::Spam => Slot::Bin,
            Self::MarkPaid => Slot::Money,
        }
    }
}

/// Why a rule is not valid.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Invalid {
    /// No condition, or more than [`MAX_CONDITIONS`].
    #[error("a rule needs one to four conditions")]
    Conditions,
    /// No action, or more than [`MAX_ACTIONS`].
    #[error("a rule needs one to four actions")]
    Actions,
    /// Two actions fill the same slot.
    #[error("two actions do the same kind of thing")]
    SameSlot,
    /// A value is empty or a stream is unknown.
    #[error("a condition or action has no usable value")]
    Value,
    /// A domain of a free mail provider, on its own.
    #[error("a free mail domain alone would match people")]
    FreeMailDomain,
    /// Neither the sender, a list, a header nor the subject is tested.
    #[error("a rule must say which mail it is about")]
    TooBroad,
    /// The trash or spam for a whole domain: people the inbox writes to may
    /// send from it.
    #[error("a whole domain is never put in the trash or spam")]
    DomainToBin,
}

/// Free mail providers: a domain of theirs on its own would match people.
const FREE_MAIL: [&str; 52] = [
    "gmail.com",
    "googlemail.com",
    "outlook.com",
    "outlook.pt",
    "outlook.es",
    "outlook.fr",
    "hotmail.com",
    "hotmail.co.uk",
    "hotmail.fr",
    "hotmail.es",
    "hotmail.it",
    "hotmail.de",
    "live.com",
    "live.co.uk",
    "live.fr",
    "msn.com",
    "yahoo.com",
    "yahoo.co.uk",
    "yahoo.fr",
    "yahoo.es",
    "yahoo.de",
    "ymail.com",
    "rocketmail.com",
    "icloud.com",
    "me.com",
    "mac.com",
    "aol.com",
    "aol.co.uk",
    "proton.me",
    "protonmail.com",
    "pm.me",
    "gmx.com",
    "gmx.net",
    "gmx.de",
    "gmx.fr",
    "web.de",
    "t-online.de",
    "yandex.com",
    "yandex.ru",
    "mail.ru",
    "fastmail.com",
    "fastmail.fm",
    "zoho.com",
    "zohomail.com",
    "tutanota.com",
    "tuta.io",
    "sapo.pt",
    "orange.fr",
    "free.fr",
    "laposte.net",
    "mail.com",
    "hey.com",
];

/// Whether `domain` is a domain someone owns: two labels or more, and not a
/// public suffix (`co.uk`).
pub fn is_owned_domain(domain: &str) -> bool {
    let domain = domain.trim().trim_matches('.').to_ascii_lowercase();
    domain.split('.').filter(|l| !l.is_empty()).count() >= 2 && !is_public_suffix(&domain)
}

/// Whether `domain` belongs to a free mail provider.
pub fn is_free_mail(domain: &str) -> bool {
    let domain = domain.trim().to_ascii_lowercase();
    FREE_MAIL.contains(&domain.as_str())
}

/// One taught rule, as the engine sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaughtRule {
    /// Stable id (the owning module's), used last to break ties.
    pub id: String,
    /// All must hold.
    pub conditions: Vec<Condition>,
    /// What it does.
    pub actions: Vec<Action>,
    /// When a member confirmed it (unix milliseconds); later wins ties.
    pub confirmed_at: i64,
}

impl TaughtRule {
    /// The sum of its conditions' weights.
    pub fn specificity(&self) -> u32 {
        self.conditions.iter().map(Condition::weight).sum()
    }

    /// Checks the shape of a rule before it is stored.
    ///
    /// # Errors
    /// The first problem found.
    pub fn validate(conditions: &[Condition], actions: &[Action]) -> Result<(), Invalid> {
        if conditions.is_empty() || conditions.len() > MAX_CONDITIONS {
            return Err(Invalid::Conditions);
        }
        if actions.is_empty() || actions.len() > MAX_ACTIONS {
            return Err(Invalid::Actions);
        }
        let mut slots = BTreeSet::new();
        for action in actions {
            if action.slot() != Slot::Label && !slots.insert(action.slot()) {
                return Err(Invalid::SameSlot);
            }
            let ok = match action {
                Action::MoveToStream { stream } => triage::STREAMS.contains(&stream.as_str()),
                Action::AddLabel { label } => !label.trim().is_empty(),
                _ => true,
            };
            if !ok {
                return Err(Invalid::Value);
            }
        }
        for condition in conditions {
            let ok = match condition {
                Condition::SenderAddress(v) => v.contains('@') && !v.trim().is_empty(),
                Condition::SenderDomain(v) => !v.contains('@') && is_owned_domain(v),
                Condition::ListId(v) | Condition::SubjectTemplate(v) | Condition::Label(v) => {
                    !v.trim().is_empty()
                }
                Condition::SubjectContains(v) => !subject_words(v).is_empty(),
                Condition::Header(test) => test.value.as_ref().is_none_or(|v| !v.trim().is_empty()),
                Condition::Stream(v) => triage::STREAMS.contains(&v.as_str()),
                Condition::SenderChecksFail => true,
            };
            if !ok {
                return Err(Invalid::Value);
            }
        }
        let narrows = |c: &Condition| {
            matches!(
                c,
                Condition::SenderAddress(_)
                    | Condition::ListId(_)
                    | Condition::SubjectTemplate(_)
                    | Condition::SubjectContains(_)
                    | Condition::Header(_)
            )
        };
        let domains: Vec<&String> = conditions
            .iter()
            .filter_map(|c| match c {
                Condition::SenderDomain(d) => Some(d),
                _ => None,
            })
            .collect();
        if !conditions.iter().any(narrows) {
            if domains.is_empty() {
                return Err(Invalid::TooBroad);
            }
            if domains.iter().any(|d| is_free_mail(&registrable_domain(d))) {
                return Err(Invalid::FreeMailDomain);
            }
        }
        // The trash and spam need more than a domain: a narrowing condition,
        // or "fails its sender checks" (mail pretending to be it).
        let bins = actions
            .iter()
            .any(|a| matches!(a, Action::Trash | Action::Spam));
        if bins
            && !conditions
                .iter()
                .any(|c| narrows(c) || *c == Condition::SenderChecksFail)
        {
            return Err(Invalid::DomainToBin);
        }
        Ok(())
    }

    /// Whether every condition holds for `facts`.
    pub fn matches(&self, facts: &Facts<'_>) -> bool {
        !self.conditions.is_empty() && self.conditions.iter().all(|c| c.holds(facts))
    }

    /// Precedence: more specific first, then later confirmed, then smaller id.
    fn precedes(&self, other: &Self) -> Ordering {
        other
            .specificity()
            .cmp(&self.specificity())
            .then(other.confirmed_at.cmp(&self.confirmed_at))
            .then(self.id.cmp(&other.id))
    }
}

/// What the engine reads of one message and its conversation.
#[derive(Debug, Clone, Default)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "independent facts about one message, each read by its own condition or guard"
)]
pub struct Facts<'a> {
    /// The sender's address.
    pub sender_address: &'a str,
    /// The subject.
    pub subject: &'a str,
    /// Its template, when it has one.
    pub subject_template: Option<&'a str>,
    /// `List-Id`.
    pub list_id: Option<&'a str>,
    /// `List-Unsubscribe`.
    pub list_unsubscribe: Option<&'a str>,
    /// `Precedence`.
    pub precedence: Option<&'a str>,
    /// `Auto-Submitted`.
    pub auto_submitted: Option<&'a str>,
    /// The conversation's label ids.
    pub labels: &'a [String],
    /// The conversation's stream.
    pub stream: &'a str,
    /// The message looks suspicious, or its conversation has an open
    /// Review need.
    pub suspicious: bool,
    /// The receiving provider's authentication results say the sender's
    /// domain failed its checks (`EmailFacts::authentication_failed`): the
    /// broad test, which keeps `mark_paid` off such mail.
    pub auth_failed: bool,
    /// They hold an explicit DMARC failure under a published reject or
    /// quarantine policy (`signals::facts::dmarc_refused`): the narrow test
    /// "fails its sender checks" reads, since the domain's genuine mail
    /// passes.
    pub impersonated: bool,
    /// The inbox writes to the sender: "fails its sender checks" never
    /// matches their mail.
    pub correspondent: bool,
}

/// One slot's answer and the rule that gave it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Applied<T> {
    /// The value.
    pub value: T,
    /// The rule's id.
    pub rule: String,
}

/// What the matching rules say about one message.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Outcome {
    /// The stream to put it in.
    pub stream: Option<Applied<String>>,
    /// Archive it.
    pub archive: Option<String>,
    /// Mark it read.
    pub read: Option<String>,
    /// Labels to add, each with its rule.
    pub labels: Vec<Applied<String>>,
    /// "Needs a reply".
    pub needs_reply: Option<Applied<bool>>,
    /// "Asks for an action".
    pub asks_action: Option<Applied<bool>>,
    /// Always or never show.
    pub surface: Option<Applied<Surface>>,
    /// Into the trash or spam.
    pub bin: Option<Applied<Bin>>,
    /// Its bills are paid.
    pub paid: Option<String>,
}

impl Outcome {
    /// Whether no rule said anything.
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    /// Every rule that contributed, once each, in a stable order.
    pub fn rules(&self) -> BTreeSet<&str> {
        let mut out = BTreeSet::new();
        out.extend(self.stream.as_ref().map(|a| a.rule.as_str()));
        out.extend(self.archive.as_deref());
        out.extend(self.read.as_deref());
        out.extend(self.labels.iter().map(|a| a.rule.as_str()));
        out.extend(self.needs_reply.as_ref().map(|a| a.rule.as_str()));
        out.extend(self.asks_action.as_ref().map(|a| a.rule.as_str()));
        out.extend(self.surface.as_ref().map(|a| a.rule.as_str()));
        out.extend(self.bin.as_ref().map(|a| a.rule.as_str()));
        out.extend(self.paid.as_deref());
        out
    }
}

/// What the active `rules` say about a message with `facts`.
pub fn evaluate(rules: &[TaughtRule], facts: &Facts<'_>) -> Outcome {
    let mut matching: Vec<&TaughtRule> = rules.iter().filter(|r| r.matches(facts)).collect();
    matching.sort_by(|a, b| a.precedes(b));

    let mut out = Outcome::default();
    for rule in &matching {
        for action in &rule.actions {
            let id = || rule.id.clone();
            match action {
                Action::MoveToStream { stream } if out.stream.is_none() => {
                    out.stream = Some(Applied {
                        value: stream.clone(),
                        rule: id(),
                    });
                }
                Action::Archive if out.archive.is_none() => out.archive = Some(id()),
                Action::MarkRead if out.read.is_none() => out.read = Some(id()),
                Action::AddLabel { label } if !out.labels.iter().any(|l| &l.value == label) => {
                    out.labels.push(Applied {
                        value: label.clone(),
                        rule: id(),
                    });
                }
                Action::NeedsReply { value } if out.needs_reply.is_none() => {
                    out.needs_reply = Some(Applied {
                        value: *value,
                        rule: id(),
                    });
                }
                Action::AsksAction { value } if out.asks_action.is_none() => {
                    out.asks_action = Some(Applied {
                        value: *value,
                        rule: id(),
                    });
                }
                Action::Surface { value } if out.surface.is_none() => {
                    out.surface = Some(Applied {
                        value: *value,
                        rule: id(),
                    });
                }
                Action::Trash | Action::Spam if out.bin.is_none() => {
                    let value = if *action == Action::Trash {
                        Bin::Trash
                    } else {
                        Bin::Spam
                    };
                    out.bin = Some(Applied { value, rule: id() });
                }
                Action::MarkPaid if out.paid.is_none() => out.paid = Some(id()),
                _ => {}
            }
        }
    }

    let always = out
        .surface
        .as_ref()
        .is_some_and(|s| s.value == Surface::Always);
    if always {
        // "Never hide this" outranks the trash and spam too.
        out.bin = None;
    }
    let keep_visible = facts.suspicious || always;
    if keep_visible {
        out.archive = None;
        out.read = None;
        if out.stream.as_ref().is_some_and(|s| s.value != "primary") {
            out.stream = None;
        }
    }
    if facts.suspicious {
        if out.needs_reply.as_ref().is_some_and(|a| !a.value) {
            out.needs_reply = None;
        }
        if out.asks_action.as_ref().is_some_and(|a| !a.value) {
            out.asks_action = None;
        }
        if out
            .surface
            .as_ref()
            .is_some_and(|s| s.value == Surface::Never)
        {
            out.surface = None;
        }
        out.paid = None;
    }
    // A bill whose sender failed its checks may be a fake one: never marked
    // paid without the member, whatever the model made of it.
    if facts.auth_failed {
        out.paid = None;
    }
    out
}

/// A `List-Id` as rules compare it: lower-cased, the id inside its angle
/// brackets (`Design Notes <digest.design-notes.example>` →
/// `digest.design-notes.example`).
pub fn normalise_list_id(value: &str) -> String {
    let value = value.trim().to_ascii_lowercase();
    match (value.rfind('<'), value.rfind('>')) {
        (Some(start), Some(end)) if start < end => value[start + 1..end].trim().to_owned(),
        _ => value,
    }
}

fn contains_words(subject: &str, phrase: &str) -> bool {
    let want = subject_words(phrase);
    if want.is_empty() {
        return false;
    }
    let have = subject_words(subject);
    have.windows(want.len()).any(|w| w == want.as_slice())
}

/// The active rules of one inbox as a [`Strategy`] for the `stream`,
/// `needs_reply` and `asks_action` questions, reading the envelope alone (a
/// new conversation: no labels, Primary, not suspicious). It abstains on every
/// other question and where no rule matches.
pub struct TaughtRules {
    rules: Vec<TaughtRule>,
}

impl TaughtRules {
    /// A strategy over `rules`.
    pub fn new(rules: Vec<TaughtRule>) -> Self {
        Self { rules }
    }
}

/// The rule id a taught answer's [`Reason`] names.
pub const REASON: &str = "taught.rule";

impl Strategy for TaughtRules {
    fn name(&self) -> &'static str {
        "taught"
    }

    fn decide<'a>(
        &'a self,
        envelope: &'a Envelope,
        questions: &'a [Question],
    ) -> BoxFuture<'a, Result<Decisions, Error>> {
        Box::pin(async move {
            let started = Instant::now();
            let template = super::signals::email::subject_template(&envelope.subject);
            let facts = Facts {
                sender_address: &envelope.from.address,
                subject: &envelope.subject,
                subject_template: template.as_deref(),
                list_id: envelope.header("list-id"),
                list_unsubscribe: envelope.header("list-unsubscribe"),
                precedence: envelope.header("precedence"),
                auto_submitted: envelope.header("auto-submitted"),
                labels: &[],
                stream: "primary",
                suspicious: false,
                auth_failed: false,
                impersonated: false,
                correspondent: false,
            };
            let outcome = evaluate(&self.rules, &facts);
            let answers = questions
                .iter()
                .map(|q| {
                    let (index, rule) = match (q.id.as_str(), &q.kind) {
                        (triage::STREAM, QuestionKind::Choice { options, .. }) => {
                            let applied = outcome.stream.as_ref()?;
                            let index = options.iter().position(|o| o.label == applied.value)?;
                            (index, applied.rule.clone())
                        }
                        (triage::NEEDS_REPLY, _) => {
                            let a = outcome.needs_reply.as_ref()?;
                            (usize::from(a.value), a.rule.clone())
                        }
                        (triage::ASKS_ACTION, _) => {
                            let a = outcome.asks_action.as_ref()?;
                            (usize::from(a.value), a.rule.clone())
                        }
                        _ => return None,
                    };
                    Some(Decided {
                        answer: local_answer(
                            q,
                            peaked(q.option_count(), index, 1.0),
                            "taught",
                            started,
                        ),
                        reasons: vec![Reason::new(REASON, rule)],
                    })
                })
                .collect();
            Ok(Decisions {
                answers,
                remote_calls: 0,
            })
        })
    }
}

#[cfg(test)]
mod tests;
