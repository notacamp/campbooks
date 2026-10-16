//! Strategies: interchangeable ways of answering the same typed questions
//! about a message (ADR 0009, campbooks-cloud#59).
//!
//! A [`Strategy`] takes an [`Envelope`] and the questions a module asks of
//! it, and returns, per question, either an [`Answer`] with the
//! [`Reason`]s behind it, or nothing (it abstains). The answer type is the one
//! [`Gateway::decide`] returns, so a caller gates every strategy the same way
//! with [`Answer::accept_if`].
//!
//! - [`EmailFacts`](super::signals::facts::EmailFacts): structured facts
//!   (headers, addresses), which only ever say "no" (ADR 0018);
//! - [`Knn`](super::knn::Knn): the nearest of the workspace's own
//!   corrections;
//! - [`Model`]: the `decide` role (Laya or an instruct model);
//! - [`Cascade`]: strategies in order, each trusted above its own threshold
//!   per question.
//!
//! Which strategy answers which question, and from which confidence an
//! answer may act, is measured by `cargo xtask eval decide` on the eval set
//! in `crates/ai/evals/decide/`.

use std::{collections::HashMap, fmt, future::Future, pin::Pin, sync::Arc, time::Instant};

use super::{Answer, AnswerValue, DecisionContext, Question, signals::email::Envelope};
use crate::{Error, Gateway};

/// A boxed, sendable future, so strategies can be used as trait objects.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Why a strategy answered as it did: a stable rule id and the evidence it
/// matched. The UI shows it under "Why?", translated by `rule`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reason {
    /// Stable id, `<strategy>.<rule>` (`facts.list_mail`,
    /// `knn.neighbours`, `model.decide`).
    pub rule: &'static str,
    /// What matched: a header name, an address, a count.
    pub evidence: String,
}

impl Reason {
    /// A reason for `rule` with `evidence`.
    pub fn new(rule: &'static str, evidence: impl Into<String>) -> Self {
        Self {
            rule,
            evidence: evidence.into(),
        }
    }
}

impl fmt::Display for Reason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.evidence.is_empty() {
            f.write_str(self.rule)
        } else {
            write!(f, "{}: {}", self.rule, self.evidence)
        }
    }
}

/// One question's answer and the reasons for it.
#[derive(Debug, Clone, PartialEq)]
pub struct Decided {
    /// The answer, as [`Gateway::decide`] would return it.
    pub answer: Answer,
    /// Why; may be empty for a model answer.
    pub reasons: Vec<Reason>,
}

/// What a strategy returns for one envelope.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Decisions {
    /// One entry per question, in question order; `None` where the strategy
    /// abstained.
    pub answers: Vec<Option<Decided>>,
    /// Calls made to a model server (`decide` or `embed`) for this envelope:
    /// what a cascade saves.
    pub remote_calls: u32,
}

impl Decisions {
    /// `n` abstentions and no call.
    pub fn abstain(n: usize) -> Self {
        Self {
            answers: vec![None; n],
            remote_calls: 0,
        }
    }

    /// The answer to question `id`, if any, given the questions asked.
    pub fn get<'a>(&'a self, questions: &[Question], id: &str) -> Option<&'a Decided> {
        let index = questions.iter().position(|q| q.id == id)?;
        self.answers.get(index)?.as_ref()
    }
}

/// A way of answering typed questions about a message.
pub trait Strategy: Send + Sync {
    /// Short name, used in reports and traces (`facts`, `knn-lexical`, …).
    fn name(&self) -> &str;

    /// Answer `questions` about `envelope`; abstain (`None`) on any question it
    /// cannot answer. Answers come back in question order.
    fn decide<'a>(
        &'a self,
        envelope: &'a Envelope,
        questions: &'a [Question],
    ) -> BoxFuture<'a, Result<Decisions, Error>>;
}

/// The `decide` role as a strategy: one [`Gateway::decide`] call with the
/// envelope's [`decision_state`](Envelope::decision_state).
pub struct Model {
    name: String,
    gateway: Gateway,
    purpose: &'static str,
    checked: bool,
}

impl Model {
    /// A strategy named `name` asking `gateway`'s `decide` role, filed under
    /// `purpose` (`<module>.<decision>`).
    pub fn new(name: impl Into<String>, gateway: Gateway, purpose: &'static str) -> Self {
        Self {
            name: name.into(),
            gateway,
            purpose,
            checked: false,
        }
    }

    /// The same strategy, answering only while the role's answers check
    /// ([`super::check`]) passes, without waiting for one still running:
    /// otherwise it fails with
    /// [`Error::AnswersNotTrusted`] before calling the model, and the
    /// answers it gives carry the check's [`stamp`](super::check::stamp) as
    /// their `model_version`. The check itself runs elsewhere
    /// ([`Gateway::keep_answers_checked`]).
    #[must_use]
    pub fn checked(mut self) -> Self {
        self.checked = true;
        self
    }
}

impl Strategy for Model {
    fn name(&self) -> &str {
        &self.name
    }

    fn decide<'a>(
        &'a self,
        envelope: &'a Envelope,
        questions: &'a [Question],
    ) -> BoxFuture<'a, Result<Decisions, Error>> {
        Box::pin(async move {
            if questions.is_empty() {
                return Ok(Decisions::default());
            }
            if self.checked && !self.gateway.answers_trusted() {
                return Err(Error::AnswersNotTrusted);
            }
            let answers = self
                .gateway
                .decide(
                    &DecisionContext::new(self.purpose),
                    &envelope.decision_state(),
                    questions,
                )
                .await?;
            Ok(Decisions {
                answers: answers
                    .iter()
                    .map(|answer| {
                        let mut answer = answer.clone();
                        if self.checked {
                            answer.model_version =
                                Some(super::check::stamp(answer.model_version.as_deref()));
                        }
                        Some(Decided {
                            reasons: vec![Reason::new("model.decide", answer.model.clone())],
                            answer,
                        })
                    })
                    .collect(),
                remote_calls: 1,
            })
        })
    }
}

/// One stage of a [`Cascade`]: a strategy and, per question id, the
/// confidence from which its answer is taken. A question without a threshold
/// is never asked of this stage.
pub struct Stage {
    /// The strategy.
    pub strategy: Arc<dyn Strategy>,
    /// Question id → lowest confidence accepted.
    pub thresholds: HashMap<String, f64>,
}

/// Strategies tried in order: for each question, the first stage whose answer
/// reaches that stage's threshold wins. Later stages are asked only the
/// questions still open, and not at all when none is. A stage that fails (a
/// model server down) abstains, so the answers already found stand.
pub struct Cascade {
    name: String,
    stages: Vec<Stage>,
}

impl Cascade {
    /// A cascade of `stages`, named after them (`facts>knn-lexical>jev`).
    pub fn new(stages: Vec<Stage>) -> Self {
        let name = stages
            .iter()
            .map(|s| s.strategy.name())
            .collect::<Vec<_>>()
            .join(">");
        Self { name, stages }
    }
}

impl Strategy for Cascade {
    fn name(&self) -> &str {
        &self.name
    }

    fn decide<'a>(
        &'a self,
        envelope: &'a Envelope,
        questions: &'a [Question],
    ) -> BoxFuture<'a, Result<Decisions, Error>> {
        Box::pin(async move {
            let mut out = Decisions::abstain(questions.len());
            for stage in &self.stages {
                let open: Vec<usize> = (0..questions.len())
                    .filter(|&i| {
                        out.answers[i].is_none() && stage.thresholds.contains_key(&questions[i].id)
                    })
                    .collect();
                if open.is_empty() {
                    continue;
                }
                let asked: Vec<Question> = open.iter().map(|&i| questions[i].clone()).collect();
                let decided = match stage.strategy.decide(envelope, &asked).await {
                    Ok(decided) => decided,
                    Err(error) => {
                        // A stage that fails abstains: the earlier stages'
                        // answers stand and the rest are asked of the user.
                        tracing::warn!(stage = stage.strategy.name(), %error, "decide stage failed");
                        out.remote_calls += 1;
                        continue;
                    }
                };
                out.remote_calls += decided.remote_calls;
                for (&i, answer) in open.iter().zip(decided.answers) {
                    let threshold = stage.thresholds[&questions[i].id];
                    if let Some(decided) = answer
                        && decided.answer.accept_if(threshold).is_some()
                    {
                        out.answers[i] = Some(decided);
                    }
                }
            }
            Ok(out)
        })
    }
}

/// An [`Answer`] built by a local strategy (facts, k-NN): probabilities in
/// question order, confidence = the largest, a score's value = the expected
/// level, as [`Gateway::decide`] does.
pub(crate) fn local_answer(
    question: &Question,
    probabilities: Vec<f64>,
    model: &str,
    started: Instant,
) -> Answer {
    let confidence = probabilities.iter().copied().reduce(f64::max);
    let best = probabilities
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.total_cmp(b.1))
        .map_or(0, |(i, _)| i);
    let value = match &question.kind {
        super::QuestionKind::Choice { options, .. } => {
            AnswerValue::Choice(options[best].label.clone())
        }
        super::QuestionKind::Score { .. } => AnswerValue::Score(
            probabilities
                .iter()
                .zip(0u32..)
                .map(|(p, i)| f64::from(i) * p)
                .sum(),
        ),
        super::QuestionKind::Noul { .. } => AnswerValue::Noul(best == 1),
    };
    Answer {
        question_id: question.id.clone(),
        value,
        probabilities: Some(probabilities),
        confidence,
        model: model.to_owned(),
        model_version: None,
        latency: started.elapsed(),
    }
}

/// Probabilities for `n` options that put `confidence` on `index` and share
/// the rest evenly.
pub(crate) fn peaked(n: usize, index: usize, confidence: f64) -> Vec<f64> {
    if n <= 1 {
        return vec![1.0; n];
    }
    let rest = (1.0 - confidence) / count_f64(n - 1);
    (0..n)
        .map(|i| if i == index { confidence } else { rest })
        .collect()
}

/// A count as `f64`. Counts here (options, neighbours, items) are tiny, far
/// below 2^52, so the conversion is exact.
#[allow(clippy::cast_precision_loss)]
pub(crate) fn count_f64(n: usize) -> f64 {
    n as f64
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU32, Ordering};

    use super::*;
    use crate::decide::signals::email::Party;

    /// Answers every question with a fixed confidence on option 0, counting
    /// how often it is asked.
    struct Fixed {
        name: &'static str,
        confidence: f64,
        asked: AtomicU32,
    }

    impl Strategy for Fixed {
        fn name(&self) -> &str {
            self.name
        }
        fn decide<'a>(
            &'a self,
            _: &'a Envelope,
            questions: &'a [Question],
        ) -> BoxFuture<'a, Result<Decisions, Error>> {
            self.asked.fetch_add(1, Ordering::SeqCst);
            Box::pin(async move {
                Ok(Decisions {
                    answers: questions
                        .iter()
                        .map(|q| {
                            Some(Decided {
                                answer: local_answer(
                                    q,
                                    peaked(q.option_count(), 0, self.confidence),
                                    self.name,
                                    Instant::now(),
                                ),
                                reasons: vec![Reason::new("test.fixed", self.name)],
                            })
                        })
                        .collect(),
                    remote_calls: 1,
                })
            })
        }
    }

    fn fixed(name: &'static str, confidence: f64) -> Arc<Fixed> {
        Arc::new(Fixed {
            name,
            confidence,
            asked: AtomicU32::new(0),
        })
    }

    fn envelope() -> Envelope {
        Envelope {
            from: Party {
                name: "Ana".into(),
                address: "ana@example.com".into(),
            },
            ..Envelope::default()
        }
    }

    fn questions() -> Vec<Question> {
        vec![
            Question::noul("a", "A holds."),
            Question::noul("b", "B holds."),
        ]
    }

    fn thresholds(pairs: &[(&str, f64)]) -> HashMap<String, f64> {
        pairs.iter().map(|(k, v)| ((*k).to_owned(), *v)).collect()
    }

    #[tokio::test]
    async fn cascade_takes_the_first_confident_stage_per_question() {
        let first = fixed("first", 0.95);
        let second = fixed("second", 0.6);
        let cascade = Cascade::new(vec![
            Stage {
                strategy: first.clone(),
                thresholds: thresholds(&[("a", 0.9), ("b", 0.99)]),
            },
            Stage {
                strategy: second.clone(),
                thresholds: thresholds(&[("a", 0.0), ("b", 0.0)]),
            },
        ]);
        assert_eq!(cascade.name(), "first>second");
        let qs = questions();
        let out = cascade.decide(&envelope(), &qs).await.unwrap();
        assert_eq!(out.get(&qs, "a").unwrap().answer.model, "first");
        assert_eq!(out.get(&qs, "b").unwrap().answer.model, "second");
        assert_eq!(out.remote_calls, 2);
    }

    #[tokio::test]
    async fn cascade_skips_a_stage_when_every_question_is_decided() {
        let first = fixed("first", 0.95);
        let second = fixed("second", 0.6);
        let cascade = Cascade::new(vec![
            Stage {
                strategy: first.clone(),
                thresholds: thresholds(&[("a", 0.9), ("b", 0.9)]),
            },
            Stage {
                strategy: second.clone(),
                thresholds: thresholds(&[("a", 0.0), ("b", 0.0)]),
            },
        ]);
        let out = cascade.decide(&envelope(), &questions()).await.unwrap();
        assert!(out.answers.iter().all(Option::is_some));
        assert_eq!(second.asked.load(Ordering::SeqCst), 0, "no model call");
        assert_eq!(out.remote_calls, 1);
    }

    struct Failing;

    impl Strategy for Failing {
        fn name(&self) -> &'static str {
            "failing"
        }
        fn decide<'a>(
            &'a self,
            _: &'a Envelope,
            _: &'a [Question],
        ) -> BoxFuture<'a, Result<Decisions, Error>> {
            Box::pin(async { Err(Error::InvalidResponse("down".into())) })
        }
    }

    #[tokio::test]
    async fn a_failing_stage_abstains_and_earlier_answers_stand() {
        let cascade = Cascade::new(vec![
            Stage {
                strategy: fixed("first", 0.95),
                thresholds: thresholds(&[("a", 0.9)]),
            },
            Stage {
                strategy: Arc::new(Failing),
                thresholds: thresholds(&[("a", 0.0), ("b", 0.0)]),
            },
        ]);
        let qs = questions();
        let out = cascade.decide(&envelope(), &qs).await.unwrap();
        assert!(out.get(&qs, "a").is_some());
        assert!(out.get(&qs, "b").is_none());
    }

    #[tokio::test]
    async fn cascade_abstains_when_no_stage_is_sure_enough() {
        let only = fixed("only", 0.6);
        let cascade = Cascade::new(vec![Stage {
            strategy: only,
            thresholds: thresholds(&[("a", 0.9)]),
        }]);
        let out = cascade.decide(&envelope(), &questions()).await.unwrap();
        assert_eq!(out.answers, vec![None, None]);
    }

    #[test]
    fn local_answers_follow_the_decide_conventions() {
        let score = Question::score("u", "How urgent?", ["no", "soon", "now"]);
        let a = local_answer(&score, vec![0.2, 0.2, 0.6], "m", Instant::now());
        assert!((a.score().unwrap() - 1.4).abs() < 1e-12);
        assert_eq!(a.confidence, Some(0.6));

        let noul = Question::noul("n", "x");
        assert_eq!(
            local_answer(&noul, peaked(2, 1, 0.8), "m", Instant::now()).noul(),
            Some(true)
        );
        let p = peaked(4, 2, 0.7);
        assert!((p.iter().sum::<f64>() - 1.0).abs() < 1e-12);
        assert!((p[0] - 0.1).abs() < 1e-12);
    }
}
