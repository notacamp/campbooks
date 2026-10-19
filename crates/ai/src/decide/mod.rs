//! The `decide` role: typed questions about a state, typed answers with
//! probabilities (ADR 0009).
//!
//! A caller builds a compact [`DecisionState`] (headers, sender kind, the first
//! part of a body — never a raw document) and asks [`Question`]s of three
//! kinds:
//!
//! - [`QuestionKind::Choice`]: one label out of an ordered set;
//! - [`QuestionKind::Score`]: a position on an ordered rubric (the weakest
//!   type; use sparingly);
//! - [`QuestionKind::Noul`]: how likely a statement is to be true.
//!
//! [`Gateway::decide`](crate::Gateway::decide) answers every question about one
//! state in one call. Each [`Answer`] carries the result, the probability of
//! each option (when the provider can tell), a confidence and the model that
//! answered. Callers act on an answer only above a per-question threshold
//! ([`Answer::accept_if`]) and ask the user otherwise.
//!
//! [`strategy`] puts other ways of answering the same questions behind one
//! interface: the structured facts of [`signals`] (which only say "no",
//! ADR 0018), [`knn`] over the workspace's own corrections, the model, and
//! cascades of them. [`check`] grades the model
//! on obvious fictional emails, so a model that answers nonsense is not used.

mod calibration;
pub mod check;
pub mod knn;
pub(crate) mod laya;
pub(crate) mod openai;
pub mod signals;
pub mod strategy;
pub mod taught;

use std::{collections::HashSet, fmt, time::Duration};

pub use calibration::{Calibration, temper};

use crate::Error;

/// Most options a `choice` (or levels a `score`) may have. Laya shares a fixed
/// token budget between options, so accuracy drops sharply past ~20; split a
/// larger set into a coarse choice followed by a fine one.
pub const MAX_OPTIONS: usize = 20;
/// Most questions asked about one state in one call (`laya-serve`'s limit).
pub const MAX_QUESTIONS: usize = 64;
/// Longest state, in characters (`laya-serve`'s limit). States are meant to be
/// compact summaries, far below this.
pub const MAX_STATE_CHARS: usize = 50_000;

/// What the decision is about: text (an email summary) or structured JSON.
///
/// Its `Debug` output never includes the content, so a state cannot leak into
/// logs through `{:?}`.
#[derive(Clone, PartialEq)]
pub enum DecisionState {
    /// Plain text.
    Text(String),
    /// A JSON value, read by the model as JSON.
    Json(serde_json::Value),
}

impl DecisionState {
    fn char_len(&self) -> usize {
        match self {
            Self::Text(text) => text.chars().count(),
            Self::Json(value) => value.to_string().chars().count(),
        }
    }
}

impl fmt::Debug for DecisionState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Text(_) => write!(f, "DecisionState::Text({} chars)", self.char_len()),
            Self::Json(_) => write!(f, "DecisionState::Json({} chars)", self.char_len()),
        }
    }
}

impl From<String> for DecisionState {
    fn from(text: String) -> Self {
        Self::Text(text)
    }
}

impl From<&str> for DecisionState {
    fn from(text: &str) -> Self {
        Self::Text(text.to_owned())
    }
}

impl From<serde_json::Value> for DecisionState {
    fn from(value: serde_json::Value) -> Self {
        Self::Json(value)
    }
}

/// Why a decision is being asked: a stable name such as `mail.triage`, owned
/// by the calling module. It names the decision in traces, and is the key
/// that per-question evals and calibration refits will be filed under.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecisionContext<'a> {
    /// `<module>.<decision>`.
    pub purpose: &'a str,
}

impl<'a> DecisionContext<'a> {
    /// A context for `purpose` (`<module>.<decision>`).
    pub const fn new(purpose: &'a str) -> Self {
        Self { purpose }
    }
}

/// One option of a [`QuestionKind::Choice`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChoiceOption {
    /// The answer key (`billing`, `newsletter`, …).
    pub label: String,
    /// What the option means, read by the model. May be empty.
    pub description: String,
}

/// The three question types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QuestionType {
    /// One label out of a set.
    Choice,
    /// A position on an ordered rubric.
    Score,
    /// Probability that a statement holds.
    Noul,
}

impl QuestionType {
    /// Lower-case name, as used on the wire and in calibration keys.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Choice => "choice",
            Self::Score => "score",
            Self::Noul => "noul",
        }
    }
}

/// What is asked, by type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuestionKind {
    /// Pick one option. `options` is ordered: the order is what the model
    /// reads and the order of [`Answer::probabilities`].
    Choice {
        /// The question, read by the model.
        instructions: String,
        /// The options, in order.
        options: Vec<ChoiceOption>,
    },
    /// Place the state on an ordered rubric, lowest level first.
    Score {
        /// The question, read by the model.
        instructions: String,
        /// Level descriptions, index 0 first.
        levels: Vec<String>,
    },
    /// Is this statement true of the state?
    Noul {
        /// The statement or yes/no question.
        statement: String,
    },
}

/// A typed question with an id unique within one call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Question {
    /// Answer key (`category`, `asks_user`, …).
    pub id: String,
    /// What is asked.
    pub kind: QuestionKind,
}

impl Question {
    /// A `choice` question over `(label, description)` options, in order.
    pub fn choice<L, D>(
        id: impl Into<String>,
        instructions: impl Into<String>,
        options: impl IntoIterator<Item = (L, D)>,
    ) -> Self
    where
        L: Into<String>,
        D: Into<String>,
    {
        Self {
            id: id.into(),
            kind: QuestionKind::Choice {
                instructions: instructions.into(),
                options: options
                    .into_iter()
                    .map(|(label, description)| ChoiceOption {
                        label: label.into(),
                        description: description.into(),
                    })
                    .collect(),
            },
        }
    }

    /// A `score` question over level descriptions, lowest first.
    pub fn score<S: Into<String>>(
        id: impl Into<String>,
        instructions: impl Into<String>,
        levels: impl IntoIterator<Item = S>,
    ) -> Self {
        Self {
            id: id.into(),
            kind: QuestionKind::Score {
                instructions: instructions.into(),
                levels: levels.into_iter().map(Into::into).collect(),
            },
        }
    }

    /// A `noul` question: how likely is `statement` to be true?
    pub fn noul(id: impl Into<String>, statement: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            kind: QuestionKind::Noul {
                statement: statement.into(),
            },
        }
    }

    /// The question's type.
    pub const fn question_type(&self) -> QuestionType {
        match self.kind {
            QuestionKind::Choice { .. } => QuestionType::Choice,
            QuestionKind::Score { .. } => QuestionType::Score,
            QuestionKind::Noul { .. } => QuestionType::Noul,
        }
    }

    /// How many answers the model chooses between (2 for `noul`).
    pub fn option_count(&self) -> usize {
        match &self.kind {
            QuestionKind::Choice { options, .. } => options.len(),
            QuestionKind::Score { levels, .. } => levels.len(),
            QuestionKind::Noul { .. } => 2,
        }
    }

    fn validate(&self) -> Result<(), Error> {
        let id = &self.id;
        let invalid = |msg: String| Err(Error::InvalidQuestion(format!("question `{id}`: {msg}")));
        if id.trim().is_empty() {
            return Err(Error::InvalidQuestion("a question id is empty".into()));
        }
        match &self.kind {
            QuestionKind::Choice {
                instructions,
                options,
            } => {
                if instructions.trim().is_empty() {
                    return invalid("instructions are empty".into());
                }
                if options.len() < 2 {
                    return invalid("a choice needs at least two options".into());
                }
                if options.len() > MAX_OPTIONS {
                    return invalid(format!(
                        "{} options is more than the {MAX_OPTIONS} the decide role answers \
                         reliably; split it into a coarse choice and a fine one",
                        options.len()
                    ));
                }
                let mut seen = HashSet::new();
                for option in options {
                    if option.label.trim().is_empty() {
                        return invalid("an option label is empty".into());
                    }
                    if !seen.insert(option.label.as_str()) {
                        return invalid(format!("option `{}` appears twice", option.label));
                    }
                }
            }
            QuestionKind::Score {
                instructions,
                levels,
            } => {
                if instructions.trim().is_empty() {
                    return invalid("instructions are empty".into());
                }
                if levels.len() < 2 {
                    return invalid("a score needs at least two levels".into());
                }
                if levels.len() > MAX_OPTIONS {
                    return invalid(format!(
                        "{} levels is more than the {MAX_OPTIONS} the decide role answers \
                         reliably; use fewer levels",
                        levels.len()
                    ));
                }
                if levels.iter().any(|level| level.trim().is_empty()) {
                    return invalid("a level description is empty".into());
                }
            }
            QuestionKind::Noul { statement } => {
                if statement.trim().is_empty() {
                    return invalid("the statement is empty".into());
                }
            }
        }
        Ok(())
    }
}

/// Check a whole call before anything is sent: ids unique, every question
/// answerable, the state within limits.
pub(crate) fn validate(state: &DecisionState, questions: &[Question]) -> Result<(), Error> {
    if questions.len() > MAX_QUESTIONS {
        return Err(Error::InvalidQuestion(format!(
            "{} questions in one call; the most is {MAX_QUESTIONS}",
            questions.len()
        )));
    }
    let mut ids = HashSet::new();
    for question in questions {
        question.validate()?;
        if !ids.insert(question.id.as_str()) {
            return Err(Error::InvalidQuestion(format!(
                "question id `{}` is used twice",
                question.id
            )));
        }
    }
    let len = state.char_len();
    if len > MAX_STATE_CHARS {
        return Err(Error::InvalidQuestion(format!(
            "the state is {len} characters; the most is {MAX_STATE_CHARS}. \
             Pass a compact summary, not the raw document"
        )));
    }
    Ok(())
}

/// A typed answer.
#[derive(Debug, Clone, PartialEq)]
pub enum AnswerValue {
    /// The chosen option's label.
    Choice(String),
    /// The level, 0-based. With probabilities it is the expected level
    /// (`Σ i·pᵢ`), so it can fall between levels; without, the level the model
    /// picked.
    Score(f64),
    /// Whether the statement holds (probability of true ≥ 0.5 when known).
    Noul(bool),
}

/// The answer to one question.
#[derive(Debug, Clone, PartialEq)]
pub struct Answer {
    /// The question's id.
    pub question_id: String,
    /// The answer.
    pub value: AnswerValue,
    /// Calibrated probability of each option, in question order (choice
    /// options, score levels, or `[false, true]` for `noul`). `None` when the
    /// provider cannot tell (an instruct model without log-probabilities).
    pub probabilities: Option<Vec<f64>>,
    /// Probability of the reported answer (the largest of `probabilities`).
    /// `None` when probabilities are unknown.
    pub confidence: Option<f64>,
    /// Model that answered, as the provider names it.
    pub model: String,
    /// Model revision, when the provider reports one.
    pub model_version: Option<String>,
    /// Wall-clock time of the whole call this answer came from.
    pub latency: Duration,
}

impl Answer {
    /// The value if the model is at least `threshold` sure of it; `None`
    /// below it or when confidence is unknown — ask the user instead.
    pub fn accept_if(&self, threshold: f64) -> Option<&AnswerValue> {
        match self.confidence {
            Some(confidence) if confidence >= threshold => Some(&self.value),
            _ => None,
        }
    }

    /// The chosen label, for a `choice`.
    pub fn choice(&self) -> Option<&str> {
        match &self.value {
            AnswerValue::Choice(label) => Some(label),
            _ => None,
        }
    }

    /// The level, for a `score`.
    pub fn score(&self) -> Option<f64> {
        match self.value {
            AnswerValue::Score(level) => Some(level),
            _ => None,
        }
    }

    /// Whether the statement holds, for a `noul`.
    pub fn noul(&self) -> Option<bool> {
        match self.value {
            AnswerValue::Noul(holds) => Some(holds),
            _ => None,
        }
    }

    /// Probability that a `noul` statement holds, when known.
    pub fn probability_true(&self) -> Option<f64> {
        match (&self.value, &self.probabilities) {
            (AnswerValue::Noul(_), Some(p)) => p.get(1).copied(),
            _ => None,
        }
    }
}

/// Every answer of one call, in question order (a catalogue or move question
/// left unanswered has none, see `signals::is_tolerant`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Answers {
    answers: Vec<Answer>,
}

impl Answers {
    pub(crate) fn new(answers: Vec<Answer>) -> Self {
        Self { answers }
    }

    /// The answer to question `id`.
    pub fn get(&self, id: &str) -> Option<&Answer> {
        self.answers.iter().find(|a| a.question_id == id)
    }

    /// Answers in question order.
    pub fn iter(&self) -> std::slice::Iter<'_, Answer> {
        self.answers.iter()
    }

    /// Number of answers.
    pub fn len(&self) -> usize {
        self.answers.len()
    }

    /// Whether there are none (no questions were asked).
    pub fn is_empty(&self) -> bool {
        self.answers.is_empty()
    }
}

impl<'a> IntoIterator for &'a Answers {
    type Item = &'a Answer;
    type IntoIter = std::slice::Iter<'a, Answer>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// Provider-independent tail of every decision: calibrate the raw
/// probabilities, then derive the confidence and (for `score`) the expected
/// level from them.
pub(crate) struct RawAnswer {
    pub value: AnswerValue,
    pub probabilities: Option<Vec<f64>>,
}

pub(crate) fn finish(
    question: &Question,
    raw: RawAnswer,
    calibration: &Calibration,
    model: &str,
    model_version: Option<&str>,
    latency: Duration,
) -> Answer {
    let qtype = question.question_type();
    let probabilities = raw.probabilities.map(|p| {
        let t = calibration.temperature(qtype, p.len());
        temper(&p, t)
    });
    let confidence = probabilities
        .as_ref()
        .and_then(|p| p.iter().copied().reduce(f64::max));
    let value = match (raw.value, &probabilities) {
        (AnswerValue::Score(_), Some(p)) => {
            let expected = p.iter().zip(0u32..).map(|(pi, i)| f64::from(i) * pi).sum();
            AnswerValue::Score(expected)
        }
        (AnswerValue::Noul(_), Some(p)) => AnswerValue::Noul(p.get(1).is_some_and(|t| *t >= 0.5)),
        (value, _) => value,
    };
    Answer {
        question_id: question.id.clone(),
        value,
        probabilities,
        confidence,
        model: model.to_owned(),
        model_version: model_version.map(str::to_owned),
        latency,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn answer(confidence: Option<f64>) -> Answer {
        Answer {
            question_id: "q".into(),
            value: AnswerValue::Noul(true),
            probabilities: confidence.map(|c| vec![1.0 - c, c]),
            confidence,
            model: "m".into(),
            model_version: None,
            latency: Duration::ZERO,
        }
    }

    #[test]
    fn accept_if_gates_on_confidence() {
        assert_eq!(
            answer(Some(0.9)).accept_if(0.8),
            Some(&AnswerValue::Noul(true))
        );
        assert_eq!(
            answer(Some(0.8)).accept_if(0.8),
            Some(&AnswerValue::Noul(true))
        );
        assert_eq!(answer(Some(0.7)).accept_if(0.8), None);
        assert_eq!(
            answer(None).accept_if(0.0),
            None,
            "unknown confidence never passes"
        );
    }

    #[test]
    fn typed_accessors() {
        let a = answer(Some(0.75));
        assert_eq!(a.noul(), Some(true));
        assert_eq!(a.choice(), None);
        assert_eq!(a.score(), None);
        assert_eq!(a.probability_true(), Some(0.75));
    }

    fn options(n: usize) -> Vec<(String, String)> {
        (0..n).map(|i| (format!("o{i}"), String::new())).collect()
    }

    #[test]
    fn choice_option_count_is_bounded() {
        let state = DecisionState::from("x");
        let ok = Question::choice("c", "Which?", options(MAX_OPTIONS));
        assert!(validate(&state, &[ok]).is_ok());

        let too_many = Question::choice("c", "Which?", options(MAX_OPTIONS + 1));
        let err = validate(&state, &[too_many]).unwrap_err().to_string();
        assert!(err.contains("question `c`"), "{err}");
        assert!(err.contains("21 options"), "{err}");
        assert!(err.contains("coarse choice and a fine one"), "{err}");

        let one = Question::choice("c", "Which?", options(1));
        assert!(validate(&state, &[one]).is_err());
    }

    #[test]
    fn score_level_count_is_bounded() {
        let state = DecisionState::from("x");
        let levels = |n: usize| (0..n).map(|i| format!("level {i}")).collect::<Vec<_>>();
        assert!(validate(&state, &[Question::score("s", "How?", levels(5))]).is_ok());
        assert!(validate(&state, &[Question::score("s", "How?", levels(21))]).is_err());
        assert!(validate(&state, &[Question::score("s", "How?", levels(1))]).is_err());
    }

    #[test]
    fn rejects_duplicate_ids_labels_and_empty_text() {
        let state = DecisionState::from("x");
        let dup_ids = [Question::noul("a", "x"), Question::noul("a", "y")];
        assert!(
            validate(&state, &dup_ids)
                .unwrap_err()
                .to_string()
                .contains("used twice")
        );

        let dup_labels = Question::choice("c", "Which?", [("a", ""), ("a", "")]);
        assert!(
            validate(&state, &[dup_labels])
                .unwrap_err()
                .to_string()
                .contains("twice")
        );

        assert!(validate(&state, &[Question::noul("n", " ")]).is_err());
        assert!(validate(&state, &[Question::noul("", "x")]).is_err());
        assert!(validate(&state, &[Question::choice("c", "", [("a", ""), ("b", "")])]).is_err());
    }

    #[test]
    fn rejects_oversized_calls() {
        let questions: Vec<_> = (0..=MAX_QUESTIONS)
            .map(|i| Question::noul(format!("q{i}"), "x"))
            .collect();
        assert!(validate(&DecisionState::from("x"), &questions).is_err());

        let big = DecisionState::Text("é".repeat(MAX_STATE_CHARS + 1));
        assert!(
            validate(&big, &[])
                .unwrap_err()
                .to_string()
                .contains("compact summary")
        );
        let fits = DecisionState::Text("é".repeat(MAX_STATE_CHARS));
        assert!(
            validate(&fits, &[]).is_ok(),
            "the limit counts characters, not bytes"
        );
    }

    #[test]
    fn state_debug_hides_content() {
        let text = DecisionState::from("Invoice 42 from ACME");
        assert_eq!(format!("{text:?}"), "DecisionState::Text(20 chars)");
        let json = DecisionState::from(serde_json::json!({"subject": "secret"}));
        assert!(!format!("{json:?}").contains("secret"));
    }

    #[test]
    fn finish_calibrates_and_derives_confidence_and_expected_score() {
        let q = Question::score("s", "How urgent?", ["low", "mid", "high"]);
        let raw = RawAnswer {
            value: AnswerValue::Score(2.0),
            probabilities: Some(vec![0.25, 0.25, 0.5]),
        };
        let a = finish(
            &q,
            raw,
            &Calibration::default(),
            "m",
            Some("v1"),
            Duration::ZERO,
        );
        assert_eq!(a.confidence, Some(0.5));
        assert!((a.score().unwrap() - 1.25).abs() < 1e-12);
        assert_eq!(a.model_version.as_deref(), Some("v1"));

        let hot = Calibration::from_pairs([("score", 2.0)]).unwrap();
        let raw = RawAnswer {
            value: AnswerValue::Score(2.0),
            probabilities: Some(vec![0.25, 0.25, 0.5]),
        };
        let softened = finish(&q, raw, &hot, "m", None, Duration::ZERO);
        assert!(softened.confidence.unwrap() < 0.5, "T > 1 softens");
    }

    #[test]
    fn finish_without_probabilities_keeps_the_value_and_no_confidence() {
        let q = Question::noul("n", "It is an invoice.");
        let raw = RawAnswer {
            value: AnswerValue::Noul(true),
            probabilities: None,
        };
        let a = finish(&q, raw, &Calibration::default(), "m", None, Duration::ZERO);
        assert_eq!(a.noul(), Some(true));
        assert_eq!(a.confidence, None);
        assert_eq!(a.probability_true(), None);
    }
}
