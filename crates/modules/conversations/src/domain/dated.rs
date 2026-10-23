//! Which messages the decide pass flags for a read of their dates
//! (campbooks-cloud#42).
//!
//! A message is **flagged** when the checked model answered `asks_action`,
//! `has_deadline` or `has_dated_thing` true with a confidence of at least
//! `CAMPBOOKS_CONVERSATIONS__DATED__FLAG`, its accepted `stream` answer is
//! not `newsletters`, and no accepted `suspicious` answer is true. A flag
//! acts on nothing by itself and costs no call: it only lets Scout's read
//! (`conversations.read_message`, the one `complete` call) run, and only
//! when a need opens for the message or a member opens it.
//!
//! This reads the answers the pass stored, never the message: what an email
//! means is the model's to say (ADR 0018).

use campbooks_ai::decide::signals::email::triage;

use super::needs::Answer;
use crate::{config::DatedSettings, model::NeedBasis};

/// The `newsletters` stream: list mail, never read for dates.
const NEWSLETTERS: &str = "newsletters";

/// A flagged message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Flagged {
    /// The pass answered `has_dated_thing` true at or above the threshold.
    /// Such a message the structured data did not cover is one Scout "can
    /// read for dates".
    pub by_dated_thing: bool,
    /// The kind of dated thing the pass named (`dated_kind`), when it
    /// answered and was flagged by `has_dated_thing`.
    pub kind: Option<String>,
}

/// Whether the decide pass flagged a message, from its stored `answers`
/// (the checked ones, as [`super::needs`] reads them), and why. `None`: not
/// flagged.
pub fn flag(answers: &[Answer], settings: &DatedSettings) -> Option<Flagged> {
    let said_yes = |question: &str| {
        answers.iter().any(|a| {
            a.basis == NeedBasis::Model
                && a.question == question
                && a.value.as_bool() == Some(true)
                && a.confidence.is_some_and(|c| c >= settings.flag)
        })
    };
    // Only what the policy let act counts for these two: a member's rule or
    // the model, at the confidence it was accepted from.
    let accepted = |question: &str, holds: &dyn Fn(&Answer) -> bool| {
        answers
            .iter()
            .any(|a| a.question == question && a.acted && holds(a))
    };
    if accepted(triage::STREAM, &|a| a.value.as_str() == Some(NEWSLETTERS))
        || accepted(triage::SUSPICIOUS, &|a| a.value.as_bool() == Some(true))
    {
        return None;
    }
    let by_dated_thing = settings.enabled && said_yes(triage::HAS_DATED_THING);
    if !(by_dated_thing || said_yes(triage::ASKS_ACTION) || said_yes(triage::HAS_DEADLINE)) {
        return None;
    }
    let kind = by_dated_thing
        .then(|| {
            answers
                .iter()
                .filter(|a| a.basis == NeedBasis::Model && a.question == triage::DATED_KIND)
                .find_map(|a| a.value.as_str().map(ToOwned::to_owned))
        })
        .flatten();
    Some(Flagged {
        by_dated_thing,
        kind,
    })
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;
    use crate::model::DecisionId;

    fn answer(question: &str, value: Value, confidence: f64, acted: bool) -> Answer {
        Answer {
            id: DecisionId::new(),
            question: question.to_owned(),
            basis: NeedBasis::Model,
            value,
            confidence: Some(confidence),
            acted,
            reasons: Vec::new(),
            options: Value::Null,
        }
    }

    fn settings() -> DatedSettings {
        DatedSettings::default()
    }

    #[test]
    fn a_dated_thing_at_the_threshold_flags_and_names_its_kind() {
        let answers = [
            answer(triage::STREAM, json!("updates"), 0.9, true),
            answer(triage::HAS_DATED_THING, json!(true), 0.6, false),
            answer(triage::DATED_KIND, json!("appointment"), 0.9, false),
        ];
        let flagged = flag(&answers, &settings()).unwrap();
        assert!(flagged.by_dated_thing);
        assert_eq!(flagged.kind.as_deref(), Some("appointment"));
        let below = [answer(triage::HAS_DATED_THING, json!(true), 0.59, false)];
        assert_eq!(flag(&below, &settings()), None);
        let said_no = [answer(triage::HAS_DATED_THING, json!(false), 0.99, false)];
        assert_eq!(flag(&said_no, &settings()), None);
    }

    #[test]
    fn an_action_or_a_deadline_flags_without_a_dated_thing() {
        for question in [triage::ASKS_ACTION, triage::HAS_DEADLINE] {
            let answers = [answer(question, json!(true), 0.8, true)];
            let flagged = flag(&answers, &settings()).unwrap();
            assert!(!flagged.by_dated_thing, "{question}");
            assert_eq!(flagged.kind, None);
        }
    }

    #[test]
    fn newsletters_are_never_flagged() {
        // Whatever it says about dates, mail sorted to Newsletters is not.
        let answers = [
            answer(triage::STREAM, json!("newsletters"), 0.9, true),
            answer(triage::HAS_DATED_THING, json!(true), 0.9, false),
            answer(triage::HAS_DEADLINE, json!(true), 0.9, true),
        ];
        assert_eq!(flag(&answers, &settings()), None);
        // A stream answer the policy did not accept does not veto.
        let unaccepted = [
            answer(triage::STREAM, json!("newsletters"), 0.3, false),
            answer(triage::HAS_DATED_THING, json!(true), 0.9, false),
        ];
        assert!(flag(&unaccepted, &settings()).is_some());
    }

    #[test]
    fn suspicious_mail_is_never_flagged() {
        let answers = [
            answer(triage::SUSPICIOUS, json!(true), 0.95, true),
            answer(triage::ASKS_ACTION, json!(true), 0.9, true),
            answer(triage::HAS_DATED_THING, json!(true), 0.9, false),
        ];
        assert_eq!(flag(&answers, &settings()), None);
        // Suspicion the policy did not accept is only a suggestion.
        let suggestion = [
            answer(triage::SUSPICIOUS, json!(true), 0.4, false),
            answer(triage::ASKS_ACTION, json!(true), 0.9, true),
        ];
        assert!(flag(&suggestion, &settings()).is_some());
    }

    #[test]
    fn with_dated_things_off_only_the_older_flags_remain() {
        let off = DatedSettings {
            enabled: false,
            ..DatedSettings::default()
        };
        let dated = [answer(triage::HAS_DATED_THING, json!(true), 0.99, false)];
        assert_eq!(flag(&dated, &off), None);
        let asks = [
            answer(triage::HAS_DATED_THING, json!(true), 0.99, false),
            answer(triage::ASKS_ACTION, json!(true), 0.7, true),
        ];
        let flagged = flag(&asks, &off).unwrap();
        assert!(!flagged.by_dated_thing);
    }

    #[test]
    fn only_the_model_flags() {
        let mut taught = answer(triage::ASKS_ACTION, json!(true), 1.0, true);
        taught.basis = NeedBasis::Taught;
        assert_eq!(flag(&[taught], &settings()), None);
    }

    #[test]
    fn the_threshold_is_configurable() {
        let strict = DatedSettings {
            flag: 0.9,
            ..DatedSettings::default()
        };
        let answers = [answer(triage::HAS_DEADLINE, json!(true), 0.8, true)];
        assert_eq!(flag(&answers, &strict), None);
        assert!(flag(&answers, &settings()).is_some());
    }
}
