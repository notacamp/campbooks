//! Which answers may act, per question and per source: the switch the
//! decide evals flip (campbooks-cloud#59, ADR 0018).
//!
//! [`Policy::baseline`] is the table measured by `cargo xtask eval decide`
//! on mail-triage v1 (`crates/ai/evals/decide/mail-triage/reports/baseline.json`,
//! `test` split), from two runs: `facts` (the rules, [`Source::Rules`]) and
//! `jev` (the decide model, [`Source::Model`], `typesafe/jev-1.13`). A
//! question's answers of a source act only where the operating point found
//! on `dev` *holds* its target precision on `test`, and only at or above
//! that point. Changing an entry is a reviewed code change citing a newer
//! report (ADR 0009 note); it is not an operator setting.
//!
//! The eval set is fictional: these operating points are a floor to start
//! from until an eval of real mail, from members' corrections, replaces it
//! (ADR 0018).

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
        self.accepts_confidence(answer.confidence)
    }

    /// Whether an answer with `confidence` may act (an unknown confidence
    /// never does, as [`Answer::accept_if`]).
    pub fn accepts_confidence(self, confidence: Option<f64>) -> bool {
        match (self, confidence) {
            (Self::ActAbove(threshold), Some(confidence)) => confidence >= threshold,
            _ => false,
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
    /// The measured table (mail-triage v1 baseline):
    ///
    /// | Question | Target | `facts` (rules) | `jev` (model): point, precision on `test` | Model |
    /// | --- | --- | --- | --- | --- |
    /// | `stream` | 95% | never answers | 0.49, 98% | act ≥ 0.49 |
    /// | `needs_reply` | 90% | "no" only: 0.99, 100%, act ≥ 0.99 | 0.51, 91% | act ≥ 0.51 |
    /// | `asks_action` | 90% | never answers | 0.58, 94% | act ≥ 0.58 |
    /// | `has_deadline` | 90% | never answers | 0.51, 97% | act ≥ 0.51 |
    /// | `urgency` | 85% | never answers | 0.73, 81% (does not hold) | record |
    /// | `suspicious` | 98% | never answers | 0.54, 100% | act ≥ 0.54 |
    /// | `money` | 90% | never answers | not measured | record |
    /// | `next_move` | not set | never answers | not measured | record |
    /// | `awaited` | not set | never answers | not measured | record |
    /// | `has_dated_thing` | not set | never answers | not measured | record |
    /// | `dated_kind` | not set | never answers | not measured | record |
    ///
    /// `has_dated_thing` and `dated_kind` (campbooks-cloud#42) only record
    /// until the owner's labelled dated mail gives them an operating point:
    /// they flag a message for Scout's read ([`super::dated`]) and name the
    /// kind of dated thing for the fallback, and never act on their own.
    ///
    /// `next_move` and `awaited` (campbooks-cloud#293) only record until the
    /// owner's labels give them an operating point, so no answer to either
    /// ever acts.
    ///
    /// `money` (campbooks-cloud#167) is not in the fictional set: its
    /// operating point comes from mail the owner labels
    /// (`cargo xtask eval decide --set-dir`), never from emails an agent
    /// wrote. Until then it only records, and an "owed" answer the policy
    /// does not accept raises a Pay need as a question from
    /// [`CHECK_FLOOR`](Self::CHECK_FLOOR), like any other suggestion: Pay
    /// always asks "Is it paid?", so the member decides.
    ///
    /// Without a trusted `decide` model only the facts' "no" acts: nothing
    /// is sorted out of Primary and no sure need is raised.
    pub fn baseline() -> Self {
        Self {
            entries: vec![
                (
                    questions::NEEDS_REPLY,
                    Source::Rules,
                    Switch::ActAbove(0.99),
                ),
                (questions::STREAM, Source::Model, Switch::ActAbove(0.49)),
                (
                    questions::NEEDS_REPLY,
                    Source::Model,
                    Switch::ActAbove(0.51),
                ),
                (
                    questions::ASKS_ACTION,
                    Source::Model,
                    Switch::ActAbove(0.58),
                ),
                (
                    questions::HAS_DEADLINE,
                    Source::Model,
                    Switch::ActAbove(0.51),
                ),
                (questions::SUSPICIOUS, Source::Model, Switch::ActAbove(0.54)),
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
    fn the_baseline_never_accepts_whose_move_it_is() {
        use campbooks_ai::decide::signals::moves::question;
        let policy = Policy::baseline();
        for id in question::ALL {
            for source in [Source::Model, Source::Rules] {
                assert_eq!(policy.switch(id, source), Switch::Record, "{id}");
                let confident = Answer {
                    question_id: id.into(),
                    value: AnswerValue::Choice("someone_else".into()),
                    probabilities: Some(vec![0.0, 0.0, 1.0, 0.0]),
                    confidence: Some(1.0),
                    model: "jev".into(),
                    model_version: None,
                    latency: Duration::ZERO,
                };
                assert!(!policy.switch(id, source).accepts(&confident), "{id}");
            }
        }
    }

    #[test]
    fn the_baseline_lets_the_facts_say_no_and_the_model_act() {
        let policy = Policy::baseline();
        // The facts only ever answer `needs_reply`.
        let facts_no = policy.switch(questions::NEEDS_REPLY, Source::Rules);
        assert!(facts_no.accepts(&answer(0.99)));
        assert!(!facts_no.accepts(&answer(0.98)));
        for question in [
            questions::STREAM,
            questions::ASKS_ACTION,
            questions::HAS_DEADLINE,
            questions::URGENCY,
            questions::SUSPICIOUS,
        ] {
            assert_eq!(policy.switch(question, Source::Rules), Switch::Record);
        }
        // The model sorts; its urgency does not hold its target.
        assert!(
            policy
                .switch(questions::STREAM, Source::Model)
                .accepts(&answer(0.9))
        );
        assert_eq!(
            policy.switch(questions::URGENCY, Source::Model),
            Switch::Record
        );
    }

    /// The switch the report gives a run's question: act from its operating
    /// point exactly when the precision there reaches the target on `test`.
    fn expected(question: &serde_json::Value) -> Switch {
        let target = question["target_precision"].as_f64().unwrap();
        let threshold = question["threshold"].as_f64();
        let precision = question["groups"]["all"]["at_threshold"]["precision"].as_f64();
        match (threshold, precision) {
            (Some(threshold), Some(p)) if p >= target => Switch::ActAbove(threshold),
            _ => Switch::Record,
        }
    }

    fn same(a: Switch, b: Switch) -> bool {
        match (a, b) {
            (Switch::Record, Switch::Record) => true,
            (Switch::ActAbove(x), Switch::ActAbove(y)) => (x - y).abs() < 1e-9,
            _ => false,
        }
    }

    /// The table is the eval baseline: the `facts` run gives the rules'
    /// switches and the `jev` run the model's.
    #[test]
    fn the_baseline_matches_the_committed_eval_report() {
        let path = format!(
            "{}/../../ai/evals/decide/mail-triage/reports/baseline.json",
            env!("CARGO_MANIFEST_DIR")
        );
        let report: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let run = |name: &str| {
            report["runs"]
                .as_array()
                .unwrap()
                .iter()
                .find(|run| run["name"] == name)
                .unwrap_or_else(|| panic!("the baseline has a `{name}` run"))
                .clone()
        };
        let policy = Policy::baseline();
        let asked = campbooks_ai::decide::signals::email::triage_questions();
        for (name, source) in [("facts", Source::Rules), ("jev", Source::Model)] {
            let run = run(name);
            let questions = run["questions"].as_array().unwrap();
            let measured: Vec<&str> = questions
                .iter()
                .map(|q| q["id"].as_str().unwrap())
                .collect();
            // A question the report does not measure never acts.
            for question in &asked {
                if !measured.contains(&question.id.as_str()) {
                    assert_eq!(
                        policy.switch(&question.id, source),
                        Switch::Record,
                        "{name} / {}: not measured, so it only records",
                        question.id
                    );
                }
            }
            // Changing that is a reviewed change citing the owner's eval.
            for question in [
                questions::MONEY,
                questions::HAS_DATED_THING,
                questions::DATED_KIND,
            ] {
                assert_eq!(
                    policy.switch(question, source),
                    Switch::Record,
                    "{name}: {question}"
                );
            }
            assert!(
                measured.iter().all(|id| asked.iter().any(|q| q.id == *id)),
                "{name}: the report measures a question nobody asks"
            );
            for question in questions {
                let id = question["id"].as_str().unwrap();
                let switch = policy.switch(id, source);
                assert!(
                    same(switch, expected(question)),
                    "{name} / {id}: the policy says {switch:?}, the report {:?}",
                    expected(question)
                );
            }
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
