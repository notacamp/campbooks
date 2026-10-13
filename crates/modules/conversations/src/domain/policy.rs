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

use campbooks_ai::decide::Answer;

use super::{decide::Source, questions};

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
        }
    }

    /// The same table with `switch` for `question` answered by `source`.
    #[must_use]
    pub fn with(mut self, question: &'static str, source: Source, switch: Switch) -> Self {
        self.entries
            .retain(|(q, s, _)| !(*q == question && *s == source));
        self.entries.push((question, source, switch));
        self
    }

    /// The switch for `question` answered by `source`.
    pub fn switch(&self, question: &str, source: Source) -> Switch {
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
