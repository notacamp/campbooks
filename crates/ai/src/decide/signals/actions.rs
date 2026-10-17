//! The action catalogue (campbooks-cloud#166): what Campbooks can do about
//! an email today, and how likely the member is to want each.
//!
//! Every action is something the app can actually do: reply, bring it back
//! later, open its attachment, forward it, report it as phishing, say it is
//! safe, say a bill is already paid or still to pay. Calendar actions join
//! when scheduling does (campbooks-cloud#42).
//!
//! The model only rates them: one `noul` question per action whose
//! likelihood no triage answer already gives ([`action_questions`]), asked
//! in the same call as the triage questions, each in a slot of its own.
//! [`likelihoods`] turns the answers into one likelihood per action: Reply
//! is the `needs_reply` answer, Report and It's safe the `suspicious` one
//! (and its complement), Already paid and Still to pay the
//! [`question::PAID_AUTOMATICALLY`] one (and its complement). The facts may
//! only rule an action out, never add one (ADR 0018): no attachment, no
//! Open attachment question; a no-reply sender, no Reply. Nothing here
//! reads what an email says.
//!
//! Which actions an item shows (those its need allows, above a floor, top
//! three, combos first) is the conversations module's; [`COMBOS`] is the
//! table of pairs offered as one.

use serde::{Deserialize, Serialize};

use super::{
    email::{Envelope, triage},
    facts::Facts,
};
use crate::decide::{Answer, Question};

/// The questions the catalogue adds to triage.
pub mod question {
    /// Whether the member would want it brought back later.
    pub const REMIND: &str = "action_remind";
    /// Whether the member needs to open its attachment to deal with it.
    pub const OPEN_ATTACHMENT: &str = "action_open_attachment";
    /// Whether the member would want to forward it.
    pub const FORWARD: &str = "action_forward";
    /// Whether a bill is paid automatically (direct debit, a card on file).
    pub const PAID_AUTOMATICALLY: &str = "action_paid_automatically";

    /// Every catalogue question, in the order they are asked.
    pub const ALL: [&str; 4] = [REMIND, OPEN_ATTACHMENT, FORWARD, PAID_AUTOMATICALLY];
}

/// One action of the catalogue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionId {
    /// Write back.
    Reply,
    /// Bring it back at a set time.
    Remind,
    /// Open the attachment.
    OpenAttachment,
    /// Forward it.
    Forward,
    /// Report it as phishing and delete it.
    Report,
    /// Say it is safe (clears a warning).
    Safe,
    /// The bill is already paid.
    AlreadyPaid,
    /// The bill is still to pay.
    StillToPay,
}

impl ActionId {
    /// Every action, in the catalogue's order.
    pub const ALL: [Self; 8] = [
        Self::Reply,
        Self::Remind,
        Self::OpenAttachment,
        Self::Forward,
        Self::Report,
        Self::Safe,
        Self::AlreadyPaid,
        Self::StillToPay,
    ];

    /// The wire and database name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Reply => "reply",
            Self::Remind => "remind",
            Self::OpenAttachment => "open_attachment",
            Self::Forward => "forward",
            Self::Report => "report",
            Self::Safe => "safe",
            Self::AlreadyPaid => "already_paid",
            Self::StillToPay => "still_to_pay",
        }
    }

    /// The action named `name`, if one is.
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|a| a.as_str() == name)
    }
}

/// Pairs usually done together, offered as one action in first place when
/// both clear the floor. The approved prototype's only pair is "Say yes and
/// add to calendar" (Reply with Add to calendar), which joins with the
/// calendar actions (campbooks-cloud#42); until then the table is empty and
/// the owner's labels say which pairs co-occur (`cargo xtask eval actions`
/// reports it per pair).
pub const COMBOS: &[(ActionId, ActionId)] = &[];

/// The catalogue questions to ask about `envelope`, after the facts'
/// vetoes: no attachment, no Open attachment question. Each is a `noul`
/// question of its own; none shares a slot with another or with triage.
pub fn action_questions(envelope: &Envelope) -> Vec<Question> {
    let mut questions = vec![Question::noul(
        question::REMIND,
        "Would the recipient want this email brought back to them later, for example to act on \
         it before a date or once something has happened?",
    )];
    if !envelope.attachments.is_empty() {
        questions.push(Question::noul(
            question::OPEN_ATTACHMENT,
            "Does the recipient need to open the attachment to deal with this email, for example \
             to read, sign, fill in or pay it?",
        ));
    }
    questions.push(Question::noul(
        question::FORWARD,
        "Would the recipient want to forward this email to someone else, for example to the \
         person who handles it or someone it concerns?",
    ));
    questions.push(Question::noul(
        question::PAID_AUTOMATICALLY,
        "Is this a bill that is paid automatically, for example by direct debit or a card on \
         file, so the recipient has nothing to pay by hand?",
    ));
    questions
}

/// The likelihood the member wants each action, from the answers given
/// about `envelope` (triage and catalogue questions alike, in any order).
/// An action whose answer is missing, or whose answer came without
/// probabilities, is left out (never shown on an unknown); one the facts
/// rule out is left out too.
pub fn likelihoods(envelope: &Envelope, answers: &[&Answer]) -> Vec<(ActionId, f64)> {
    // The probability that the statement of question `id` holds.
    let holds = |id: &str| {
        answers
            .iter()
            .find(|a| a.question_id == id)
            .and_then(|a| a.probabilities.as_ref())
            .and_then(|p| p.get(1).copied())
    };
    let facts = Facts::of(envelope);
    let mut out = Vec::new();
    let mut push = |action: ActionId, likelihood: Option<f64>| {
        if let Some(likelihood) = likelihood {
            out.push((action, likelihood.clamp(0.0, 1.0)));
        }
    };
    if !facts.no_reply_sender() {
        push(ActionId::Reply, holds(triage::NEEDS_REPLY));
    }
    push(ActionId::Remind, holds(question::REMIND));
    if !envelope.attachments.is_empty() {
        push(ActionId::OpenAttachment, holds(question::OPEN_ATTACHMENT));
    }
    push(ActionId::Forward, holds(question::FORWARD));
    let suspicious = holds(triage::SUSPICIOUS);
    push(ActionId::Report, suspicious);
    push(ActionId::Safe, suspicious.map(|p| 1.0 - p));
    let paid = holds(question::PAID_AUTOMATICALLY);
    push(ActionId::AlreadyPaid, paid);
    push(ActionId::StillToPay, paid.map(|p| 1.0 - p));
    out
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, time::Duration};

    use super::*;
    use crate::decide::{
        AnswerValue, DecisionState,
        signals::email::{Attachment, Party, triage_questions},
    };

    fn envelope(from: &str, attachments: usize) -> Envelope {
        Envelope {
            from: Party {
                name: "Sender".into(),
                address: from.into(),
            },
            to: vec![],
            cc: vec![],
            subject: "Subject".into(),
            headers: BTreeMap::new(),
            attachments: (0..attachments)
                .map(|n| Attachment {
                    name: format!("file-{n}.pdf"),
                    content_type: "application/pdf".into(),
                })
                .collect(),
            body: String::new(),
            recipient: None,
        }
    }

    fn noul(id: &str, holds: f64) -> Answer {
        Answer {
            question_id: id.into(),
            value: AnswerValue::Noul(holds >= 0.5),
            probabilities: Some(vec![1.0 - holds, holds]),
            confidence: Some(holds.max(1.0 - holds)),
            model: "test".into(),
            model_version: None,
            latency: Duration::ZERO,
        }
    }

    #[test]
    fn the_facts_only_rule_actions_out() {
        let none = action_questions(&envelope("ana@example.com", 0));
        assert!(none.iter().all(|q| q.id != question::OPEN_ATTACHMENT));
        let one = action_questions(&envelope("ana@example.com", 1));
        assert!(one.iter().any(|q| q.id == question::OPEN_ATTACHMENT));
        assert_eq!(one.len(), question::ALL.len());

        let answers = [
            noul(triage::NEEDS_REPLY, 0.9),
            noul(question::OPEN_ATTACHMENT, 0.8),
        ];
        let refs: Vec<&Answer> = answers.iter().collect();
        // A no-reply sender gets no Reply, whatever the model says; no
        // attachment, no Open attachment.
        let rated = likelihoods(&envelope("no-reply@shop.example", 0), &refs);
        assert!(rated.iter().all(|(a, _)| *a != ActionId::Reply));
        assert!(rated.iter().all(|(a, _)| *a != ActionId::OpenAttachment));
        let rated = likelihoods(&envelope("ana@example.com", 1), &refs);
        assert!(rated.contains(&(ActionId::Reply, 0.9)));
        assert!(rated.contains(&(ActionId::OpenAttachment, 0.8)));
    }

    #[test]
    fn pairs_come_from_one_answer_and_unknowns_are_left_out() {
        let answers = [
            noul(triage::SUSPICIOUS, 0.75),
            noul(question::PAID_AUTOMATICALLY, 0.25),
        ];
        let refs: Vec<&Answer> = answers.iter().collect();
        let rated = likelihoods(&envelope("ana@example.com", 0), &refs);
        assert!(rated.contains(&(ActionId::Report, 0.75)));
        assert!(rated.contains(&(ActionId::Safe, 0.25)));
        assert!(rated.contains(&(ActionId::AlreadyPaid, 0.25)));
        assert!(rated.contains(&(ActionId::StillToPay, 0.75)));
        // Not asked or answered: not rated.
        assert!(rated.iter().all(|(a, _)| *a != ActionId::Remind));
        // An answer without probabilities is no likelihood.
        let mut unsure = noul(question::REMIND, 0.9);
        unsure.probabilities = None;
        let rated = likelihoods(&envelope("ana@example.com", 0), &[&unsure]);
        assert!(rated.is_empty());
    }

    #[test]
    fn each_question_has_a_slot_of_its_own_and_triage_is_asked_as_before() {
        let triage = triage_questions();
        let mut all = triage.clone();
        all.extend(action_questions(&envelope("ana@example.com", 1)));
        let state = DecisionState::Text("state".into());
        let body = |questions: &[Question]| {
            serde_json::to_value(crate::decide::laya::Request {
                model: "jev",
                state: &state,
                questions,
            })
            .unwrap()
        };
        let (before, after) = (body(&triage), body(&all));
        let (before, after) = (
            before["questions"].as_object().unwrap().clone(),
            after["questions"].as_object().unwrap().clone(),
        );
        // Every triage question goes out exactly as it does without the
        // catalogue, and every catalogue question in a slot of its own.
        for (id, question) in &before {
            assert_eq!(after.get(id), Some(question), "{id}");
        }
        assert_eq!(after.len(), before.len() + question::ALL.len());
        for id in question::ALL {
            assert!(after.contains_key(id) && !before.contains_key(id), "{id}");
            assert_eq!(after[id]["type"], before["needs_reply"]["type"], "{id}");
        }
    }

    #[test]
    fn names_round_trip() {
        for action in ActionId::ALL {
            assert_eq!(ActionId::parse(action.as_str()), Some(action));
            assert_eq!(
                serde_json::to_value(action).unwrap(),
                serde_json::json!(action.as_str())
            );
        }
    }
}
