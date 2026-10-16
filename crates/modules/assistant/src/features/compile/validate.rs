//! Checking what the model proposed, on the server, before the member sees
//! it. A rule survives only when every part of it is in the taught-rule
//! vocabulary (`campbooks_ai::decide::taught`) and comes from the input: an
//! address or domain must appear in the member's sentence or be their own,
//! subject words must be in the sentence, labels must be theirs, and the
//! rule must pass the same validation `conversations.create_rule` applies.
//! Nothing is repaired by guessing: a rule that fails a check is dropped.

use std::collections::BTreeMap;

use campbooks_ai::decide::taught::{self, TaughtRule, is_free_mail};

use super::{
    Proposal, ProposalInbox, ProposedAction, ProposedCondition, ProposedHeader, WhoQuestion,
    prompt::{ModelAction, ModelCondition, ModelOutput, ModelRule, STREAMS},
    types::{CompileOutcome, HeaderKey, StreamKey, SurfaceKey, UnsupportedReason},
};

/// Most rules kept from one sentence.
pub const MAX_RULES: usize = 3;
/// Longest value of a condition or action.
const MAX_VALUE_CHARS: usize = 100;
/// Stands in for the sender a `who` question will name, while checking.
const SOMEONE: &str = "someone@example.invalid";

/// An inbox of the member, as the conversations module lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InboxFacts {
    /// Its id.
    pub id: String,
    /// Its address.
    pub address: String,
    /// Whether the member may teach rules there (`sender` or `manager`).
    pub teachable: bool,
    /// Whether someone else holds a role on it.
    pub shared: bool,
}

/// A label of one of the member's inboxes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabelFacts {
    /// Its id.
    pub id: String,
    /// Its inbox.
    pub inbox: String,
    /// Its name.
    pub name: String,
}

/// What the checks read: the member's sentence and their own data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Context {
    /// The sanitised sentence.
    pub sentence: String,
    /// The inboxes the member can read.
    pub inboxes: Vec<InboxFacts>,
    /// Their labels.
    pub labels: Vec<LabelFacts>,
}

impl Context {
    fn mentions(&self, text: &str) -> bool {
        !text.is_empty() && self.sentence.to_lowercase().contains(&text.to_lowercase())
    }

    fn own_address(&self, address: &str) -> bool {
        self.inboxes
            .iter()
            .any(|inbox| inbox.address.eq_ignore_ascii_case(address))
    }

    fn own_domain(&self, domain: &str) -> bool {
        self.inboxes.iter().any(|inbox| {
            inbox
                .address
                .rsplit_once('@')
                .is_some_and(|(_, d)| d.eq_ignore_ascii_case(domain))
        })
    }
}

/// What survived the checks, before the `who` candidates are looked up.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checked {
    /// Rules, unsupported, or unclear.
    pub outcome: CompileOutcome,
    /// Why not, when unsupported.
    pub unsupported: Option<UnsupportedReason>,
    /// The rules left, each with the inboxes it can be taught in.
    pub proposals: Vec<Proposal>,
}

/// Check `output` against the vocabulary and `ctx`.
pub fn check(output: &ModelOutput, ctx: &Context) -> Checked {
    let unsupported = match output.unsupported.as_str() {
        "sends_mail" => Some(UnsupportedReason::SendsMail),
        "deletes_mail" => Some(UnsupportedReason::DeletesMail),
        "not_about_mail" => Some(UnsupportedReason::NotAboutMail),
        "too_vague" => Some(UnsupportedReason::TooVague),
        _ => None,
    };
    let proposals: Vec<Proposal> = output
        .rules
        .iter()
        .take(MAX_RULES)
        .filter_map(|rule| draft(rule, ctx))
        .flat_map(|draft| materialise(&draft, ctx))
        .take(MAX_RULES)
        .collect();
    let outcome = if unsupported.is_some() {
        CompileOutcome::Unsupported
    } else if proposals.is_empty() {
        CompileOutcome::Unclear
    } else {
        CompileOutcome::Rules
    };
    Checked {
        outcome,
        unsupported: match outcome {
            CompileOutcome::Unsupported => unsupported,
            _ => None,
        },
        proposals,
    }
}

/// A rule whose labels are still names (they differ per inbox).
#[derive(Debug, Clone, PartialEq, Eq)]
struct Draft {
    who: Option<String>,
    conditions: Vec<Part<ProposedCondition>>,
    actions: Vec<Part<ProposedAction>>,
}

/// A part of a rule, or one that needs an inbox's label id.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Part<T> {
    Ready(T),
    Label(String),
}

fn draft(rule: &ModelRule, ctx: &Context) -> Option<Draft> {
    let mut conditions = rule
        .conditions
        .iter()
        .map(|c| condition(c, ctx))
        .collect::<Option<Vec<_>>>()?;
    let actions = rule
        .actions
        .iter()
        .map(action)
        .collect::<Option<Vec<_>>>()?;
    let who = rule.who.trim();
    let who = if who.is_empty() {
        None
    } else if !ctx.mentions(who) || who.chars().count() > MAX_VALUE_CHARS {
        return None;
    } else if let Some(sender) = sender_in(who, ctx) {
        // The phrase holds an address or domain the member wrote: no
        // question to ask (or a free mail domain: no rule).
        let Sender::Allowed(sender) = sender else {
            return None;
        };
        conditions.push(Part::Ready(sender));
        None
    } else {
        Some(who.to_owned())
    };
    Some(Draft {
        who,
        conditions,
        actions,
    })
}

/// A sender written inside a `who` phrase.
enum Sender {
    /// Usable as a condition.
    Allowed(ProposedCondition),
    /// There, but not allowed (a free mail domain).
    Refused,
}

/// An address or domain the member wrote inside a `who` phrase.
fn sender_in(phrase: &str, ctx: &Context) -> Option<Sender> {
    let token = phrase
        .split(|c: char| c.is_whitespace() || matches!(c, ',' | ';' | '(' | ')' | '<' | '>'))
        .map(|t| {
            t.trim_matches(|c: char| matches!(c, '.' | '"' | '\''))
                .to_lowercase()
        })
        .find(|t| t.contains('@') || looks_like_domain(t))?;
    let sender = if token.contains('@') {
        address(&token, ctx).map(ProposedCondition::SenderAddress)
    } else {
        domain(&token, ctx).map(ProposedCondition::SenderDomain)
    };
    Some(sender.map_or(Sender::Refused, Sender::Allowed))
}

fn condition(c: &ModelCondition, ctx: &Context) -> Option<Part<ProposedCondition>> {
    let value = c.value.trim();
    if value.chars().count() > MAX_VALUE_CHARS {
        return None;
    }
    let ready = match c.field.as_str() {
        "sender_address" => ProposedCondition::SenderAddress(address(value, ctx)?),
        "sender_domain" => ProposedCondition::SenderDomain(domain(value, ctx)?),
        "subject_contains" => {
            let words = value.split_whitespace().collect::<Vec<_>>();
            if words.is_empty() || !words.iter().all(|w| ctx.mentions(w)) {
                return None;
            }
            ProposedCondition::SubjectContains(words.join(" "))
        }
        "header_present" => ProposedCondition::Header(ProposedHeader {
            name: header(value)?,
        }),
        "stream" => ProposedCondition::Stream(stream(value)?),
        "label" => return (!value.is_empty()).then(|| Part::Label(value.to_owned())),
        _ => return None,
    };
    Some(Part::Ready(ready))
}

fn action(a: &ModelAction) -> Option<Part<ProposedAction>> {
    let value = a.value.trim();
    let ready = match a.kind.as_str() {
        "move_to_stream" => ProposedAction::MoveToStream {
            stream: stream(value)?,
        },
        "archive" => ProposedAction::Archive,
        "mark_read" => ProposedAction::MarkRead,
        "needs_reply" => ProposedAction::NeedsReply {
            value: boolean(value)?,
        },
        "asks_action" => ProposedAction::AsksAction {
            value: boolean(value)?,
        },
        "surface" => ProposedAction::Surface {
            value: match value {
                "always" => SurfaceKey::Always,
                "never" => SurfaceKey::Never,
                _ => return None,
            },
        },
        "add_label" => {
            return (!value.is_empty() && value.chars().count() <= MAX_VALUE_CHARS)
                .then(|| Part::Label(value.to_owned()));
        }
        _ => return None,
    };
    Some(Part::Ready(ready))
}

/// An address the member wrote, or one of their own.
fn address(value: &str, ctx: &Context) -> Option<String> {
    let value = value.trim().to_lowercase();
    let (local, host) = value.split_once('@')?;
    let well_formed = !local.is_empty()
        && host.contains('.')
        && !value.chars().any(|c| c.is_whitespace() || c.is_control());
    (well_formed && (ctx.mentions(&value) || ctx.own_address(&value))).then_some(value)
}

/// A domain the member wrote, or their own; never a free mail domain.
fn domain(value: &str, ctx: &Context) -> Option<String> {
    let value = value.trim().trim_start_matches('@').to_lowercase();
    (looks_like_domain(&value)
        && !is_free_mail(&value)
        && (ctx.mentions(&value) || ctx.own_domain(&value)))
    .then_some(value)
}

/// `name.tld`: letters, digits, dots and hyphens, ending in a label of two
/// or more letters.
fn looks_like_domain(value: &str) -> bool {
    value.contains('.')
        && !value.starts_with('.')
        && value
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '.' | '-'))
        && value
            .rsplit('.')
            .next()
            .is_some_and(|tld| tld.chars().count() >= 2 && tld.chars().all(char::is_alphabetic))
}

fn header(value: &str) -> Option<HeaderKey> {
    match value {
        "list_id" => Some(HeaderKey::ListId),
        "list_unsubscribe" => Some(HeaderKey::ListUnsubscribe),
        "auto_submitted" => Some(HeaderKey::AutoSubmitted),
        "precedence" => Some(HeaderKey::Precedence),
        _ => None,
    }
}

fn stream(value: &str) -> Option<StreamKey> {
    STREAMS.contains(&value).then_some(())?;
    Some(match value {
        "updates" => StreamKey::Updates,
        "receipts" => StreamKey::Receipts,
        "newsletters" => StreamKey::Newsletters,
        _ => StreamKey::Primary,
    })
}

fn boolean(value: &str) -> Option<bool> {
    match value {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

/// The draft in every inbox the member may teach in that has its labels,
/// one proposal per distinct rule (label ids differ per inbox), each
/// passing the taught-rule validation.
fn materialise(draft: &Draft, ctx: &Context) -> Vec<Proposal> {
    let mut by_rule: BTreeMap<String, Proposal> = BTreeMap::new();
    for inbox in ctx.inboxes.iter().filter(|inbox| inbox.teachable) {
        let label = |name: &str| {
            ctx.labels
                .iter()
                .find(|l| l.inbox == inbox.id && l.name.eq_ignore_ascii_case(name))
                .map(|l| l.id.clone())
        };
        let conditions = draft
            .conditions
            .iter()
            .map(|part| match part {
                Part::Ready(c) => Some(c.clone()),
                Part::Label(name) => label(name).map(ProposedCondition::Label),
            })
            .collect::<Option<Vec<_>>>();
        let actions = draft
            .actions
            .iter()
            .map(|part| match part {
                Part::Ready(a) => Some(a.clone()),
                Part::Label(name) => label(name).map(|label| ProposedAction::AddLabel { label }),
            })
            .collect::<Option<Vec<_>>>();
        let (Some(conditions), Some(actions)) = (conditions, actions) else {
            continue;
        };
        if !valid(&conditions, &actions, draft.who.is_some()) {
            continue;
        }
        let key = serde_json::to_string(&(&conditions, &actions)).unwrap_or_default();
        by_rule
            .entry(key)
            .or_insert_with(|| Proposal {
                conditions,
                actions,
                who: draft.who.clone().map(|phrase| WhoQuestion {
                    phrase,
                    candidates: Vec::new(),
                    matched: false,
                }),
                inboxes: Vec::new(),
            })
            .inboxes
            .push(ProposalInbox {
                id: inbox.id.clone(),
                address: inbox.address.clone(),
                shared: inbox.shared,
            });
    }
    by_rule.into_values().collect()
}

/// Whether `conversations.create_rule` would take it (with a sender in
/// place of the `who` still to be answered).
fn valid(conditions: &[ProposedCondition], actions: &[ProposedAction], who: bool) -> bool {
    let mut taught_conditions: Vec<taught::Condition> =
        conditions.iter().map(taught::Condition::from).collect();
    if who {
        taught_conditions.push(taught::Condition::SenderAddress(SOMEONE.to_owned()));
    }
    let taught_actions: Vec<taught::Action> = actions.iter().map(taught::Action::from).collect();
    TaughtRule::validate(&taught_conditions, &taught_actions).is_ok()
}

#[cfg(test)]
mod tests;
