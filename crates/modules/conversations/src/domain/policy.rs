//! Which answers may act, per question and per source: the switch #59's
//! evals flip. Changing an entry is a reviewed code change that cites the
//! eval report showing the source is good enough at that threshold (ADR 0009
//! note); it is not an operator setting.

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

/// The switch for `question` answered by `source`.
///
/// In this change only rules may act, and only on the stream: zero-shot
/// model answers are suggestions until a question has an eval set
/// (campbooks-cloud#59).
pub fn switch(question: &str, source: Source) -> Switch {
    match (question, source) {
        (questions::STREAM, Source::Rules) => Switch::ActAbove(0.8),
        _ => Switch::Record,
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
    fn only_confident_rules_sort_the_stream() {
        let stream = switch(questions::STREAM, Source::Rules);
        assert!(stream.accepts(&answer(0.95)));
        assert!(!stream.accepts(&answer(0.5)));
        assert!(!switch(questions::STREAM, Source::Model).accepts(&answer(0.999)));
        for question in [
            questions::NEEDS_REPLY,
            questions::ASKS_ACTION,
            questions::HAS_DEADLINE,
            questions::URGENCY,
            questions::SUSPICIOUS,
        ] {
            assert_eq!(switch(question, Source::Rules), Switch::Record);
            assert_eq!(switch(question, Source::Model), Switch::Record);
        }
    }
}
