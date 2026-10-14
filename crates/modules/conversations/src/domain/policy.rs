//! Which answers may act, per question and per source: the switch the
//! decide evals flip (campbooks-cloud#59).
//!
//! [`Policy::baseline`] is the table measured by `cargo xtask eval decide`
//! on mail-triage v1 (`crates/ai/evals/decide/mail-triage/reports/baseline.json`,
//! commit `585a6c87`, `signals` strategy, `test` split): a question's rules
//! answers act only where the operating point found on `dev` *holds* its
//! target precision on `test`, and only at or above that point. Model
//! answers have not been measured, so they never act. Changing an entry is a
//! reviewed code change citing a newer report (ADR 0009 note); it is not an
//! operator setting.

use campbooks_ai::decide::{Answer, signals::email::triage as questions};

use super::decide::Source;

/// What an answer of one source may do for one question.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Switch {
    /// Store it as a suggestion; change nothing.
    Record,
    /// Act on it when its confidence is at least this (`Answer::accept_if`).
    ActAbove(f64),
}

impl Switch {
    /// Whether `answer` may act.
    pub fn accepts(self, answer: &Answer) -> bool {
        match self {
            Self::Record => false,
            Self::ActAbove(threshold) => answer.accept_if(threshold).is_some(),
        }
    }
}

/// The switch of every (question, source); [`Switch::Record`] when absent.
#[derive(Debug, Clone, PartialEq)]
pub struct Policy {
    entries: Vec<(&'static str, Source, Switch)>,
    check_floor: f64,
}

impl Policy {
    /// The measured table (mail-triage v1 baseline, `signals`):
    ///
    /// | Question | Target | Operating point | Precision on `test` | Holds | Rules |
    /// | --- | --- | --- | --- | --- | --- |
    /// | `stream` | 95% | 0.70 | 87% | no | record |
    /// | `needs_reply` | 90% | 0.88 | 94% | yes | act ≥ 0.88 |
    /// | `asks_action` | 90% | 0.75 | 95% | yes | act ≥ 0.75 |
    /// | `has_deadline` | 90% | 0.75 | 90% | yes | act ≥ 0.75 |
    /// | `urgency` | 85% | 0.75 | 93% | yes | act ≥ 0.75 |
    /// | `suspicious` | 98% | 0.93 | 97% | no | record |
    ///
    /// So no conversation is sorted out of Primary by this table: the stream
    /// answers are kept as suggestions until a strategy holds its target.
    pub fn baseline() -> Self {
        Self {
            entries: vec![
                (
                    questions::NEEDS_REPLY,
                    Source::Rules,
                    Switch::ActAbove(0.88),
                ),
                (
                    questions::ASKS_ACTION,
                    Source::Rules,
                    Switch::ActAbove(0.75),
                ),
                (
                    questions::HAS_DEADLINE,
                    Source::Rules,
                    Switch::ActAbove(0.75),
                ),
                (questions::URGENCY, Source::Rules, Switch::ActAbove(0.75)),
            ],
            check_floor: Self::CHECK_FLOOR,
        }
    }

    /// The confidence from which a model answer the policy does not accept
    /// is put to the member as a question (a Review need, trust `check`).
    pub const CHECK_FLOOR: f64 = 0.8;

    /// [`CHECK_FLOOR`](Self::CHECK_FLOOR), unless set otherwise.
    pub const fn check_floor(&self) -> f64 {
        self.check_floor
    }

    /// The same table with `switch` for `question` answered by `source`.
    #[must_use]
    pub fn with(mut self, question: &'static str, source: Source, switch: Switch) -> Self {
        self.entries
            .retain(|(q, s, _)| !(*q == question && *s == source));
        self.entries.push((question, source, switch));
        self
    }

    /// The switch for `question` answered by `source`. A taught rule always
    /// acts: a member confirmed it, so it is not a suggestion to measure.
    pub fn switch(&self, question: &str, source: Source) -> Switch {
        if source == Source::Taught {
            return Switch::ActAbove(0.0);
        }
        self.entries
            .iter()
            .find(|(q, s, _)| *q == question && *s == source)
            .map_or(Switch::Record, |(_, _, switch)| *switch)
    }
}

impl Default for Policy {
    fn default() -> Self {
        Self::baseline()
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use campbooks_ai::decide::AnswerValue;

    use super::*;

    fn answer(confidence: f64) -> Answer {
        Answer {
            question_id: questions::STREAM.into(),
            value: AnswerValue::Choice("newsletters".into()),
            probabilities: None,
            confidence: Some(confidence),
            model: "rules".into(),
            model_version: None,
            latency: Duration::ZERO,
        }
    }

    #[test]
    fn the_baseline_lets_only_questions_that_hold_act() {
        let policy = Policy::baseline();
        // Stream and suspicious do not hold their target: suggestions only.
        assert!(
            !policy
                .switch(questions::STREAM, Source::Rules)
                .accepts(&answer(0.999))
        );
        assert_eq!(
            policy.switch(questions::SUSPICIOUS, Source::Rules),
            Switch::Record
        );
        let needs_reply = policy.switch(questions::NEEDS_REPLY, Source::Rules);
        assert!(needs_reply.accepts(&answer(0.9)));
        assert!(!needs_reply.accepts(&answer(0.87)));
        for question in [
            questions::STREAM,
            questions::NEEDS_REPLY,
            questions::ASKS_ACTION,
            questions::HAS_DEADLINE,
            questions::URGENCY,
            questions::SUSPICIOUS,
        ] {
            assert_eq!(policy.switch(question, Source::Model), Switch::Record);
        }
    }

    /// The table is the eval baseline: for the `signals` run, a question's
    /// rules act from its operating point exactly when the precision there
    /// reaches the target on `test`, and only then; no model run is in the
    /// baseline, so model answers never act.
    #[test]
    fn the_baseline_matches_the_committed_eval_report() {
        let path = format!(
            "{}/../../ai/evals/decide/mail-triage/reports/baseline.json",
            env!("CARGO_MANIFEST_DIR")
        );
        let report: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let signals = report["runs"]
            .as_array()
            .unwrap()
            .iter()
            .find(|run| run["name"] == "signals")
            .expect("the baseline has a `signals` run");
        let policy = Policy::baseline();
        let mut seen = 0;
        for question in signals["questions"].as_array().unwrap() {
            let id = question["id"].as_str().unwrap();
            let target = question["target_precision"].as_f64().unwrap();
            let threshold = question["threshold"].as_f64();
            let precision = question["groups"]["all"]["at_threshold"]["precision"].as_f64();
            let holds = matches!((threshold, precision), (Some(_), Some(p)) if p >= target);
            let expected = match threshold {
                Some(threshold) if holds => Switch::ActAbove(threshold),
                _ => Switch::Record,
            };
            assert_eq!(policy.switch(id, Source::Rules), expected, "{id}");
            assert_eq!(policy.switch(id, Source::Model), Switch::Record, "{id}");
            seen += 1;
        }
        assert_eq!(
            seen,
            campbooks_ai::decide::signals::email::triage_questions().len()
        );
        assert!(
            report["runs"]
                .as_array()
                .unwrap()
                .iter()
                .all(|run| run["offline"].as_bool().is_none_or(|offline| offline)),
            "a model run is in the baseline: give model answers their operating points"
        );
    }

    #[test]
    fn an_entry_can_be_replaced() {
        let policy =
            Policy::baseline().with(questions::STREAM, Source::Rules, Switch::ActAbove(0.9));
        assert!(
            policy
                .switch(questions::STREAM, Source::Rules)
                .accepts(&answer(0.95))
        );
        assert!(
            !policy
                .switch(questions::STREAM, Source::Rules)
                .accepts(&answer(0.85))
        );
    }
}
