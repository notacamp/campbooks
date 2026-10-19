//! `laya-serve`'s typed decision protocol: `POST <base>/systemone`.
//!
//! Request (laya 0.3.x `laya/serve.py`):
//!
//! ```json
//! {"model": "convaiinnovations/laya-multilingual",
//!  "state": "text, or any JSON value",
//!  "questions": {
//!    "<id>": {"type": "choice", "instructions": "…", "criteria": {"<label>": "<description>", …}},
//!    "<id>": {"type": "score",  "instructions": "…", "criteria": ["<level 0>", "<level 1>", …]},
//!    "<id>": {"type": "noul",   "instructions": "<statement>"}}}
//! ```
//!
//! Option order is positional for the model, so questions and criteria are
//! written in the caller's order (a `serde_json::Value` would sort them).
//!
//! Response: `{"model", "answers": {"<id>": …}, "usage", "routing": {"model",
//! "repo", "reason", …}}`, where a choice answer has `choice` and
//! `probabilities` by label, a score answer has `score` (the expected level)
//! and `probabilities` by level index, and a noul answer has `noul`
//! (probability of true). Confidence is recomputed from the probabilities
//! after calibration.

use std::collections::HashMap;

use serde::{
    Deserialize, Serialize, Serializer,
    ser::{SerializeMap, SerializeStruct},
};

use crate::{
    Error,
    decide::{AnswerValue, DecisionState, Question, QuestionKind, RawAnswer},
};

/// The request body.
pub(crate) struct Request<'a> {
    pub model: &'a str,
    pub state: &'a DecisionState,
    pub questions: &'a [Question],
}

impl Serialize for Request<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut s = serializer.serialize_struct("Request", 3)?;
        s.serialize_field("model", self.model)?;
        match self.state {
            DecisionState::Text(text) => s.serialize_field("state", text)?,
            DecisionState::Json(value) => s.serialize_field("state", value)?,
        }
        s.serialize_field("questions", &Questions(self.questions))?;
        s.end()
    }
}

struct Questions<'a>(&'a [Question]);

impl Serialize for Questions<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(self.0.len()))?;
        for question in self.0 {
            map.serialize_entry(&question.id, &WireQuestion(question))?;
        }
        map.end()
    }
}

struct WireQuestion<'a>(&'a Question);

impl Serialize for WireQuestion<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let question = self.0;
        let has_criteria = !matches!(question.kind, QuestionKind::Noul { .. });
        let mut s = serializer.serialize_struct("Question", if has_criteria { 3 } else { 2 })?;
        s.serialize_field("type", question.question_type().as_str())?;
        match &question.kind {
            QuestionKind::Choice {
                instructions,
                options,
            } => {
                s.serialize_field("instructions", instructions)?;
                s.serialize_field("criteria", &Criteria(options))?;
            }
            QuestionKind::Score {
                instructions,
                levels,
            } => {
                s.serialize_field("instructions", instructions)?;
                s.serialize_field("criteria", levels)?;
            }
            QuestionKind::Noul { statement } => {
                s.serialize_field("instructions", statement)?;
            }
        }
        s.end()
    }
}

struct Criteria<'a>(&'a [super::ChoiceOption]);

impl Serialize for Criteria<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(self.0.len()))?;
        for option in self.0 {
            map.serialize_entry(&option.label, &option.description)?;
        }
        map.end()
    }
}

#[derive(Deserialize)]
pub(crate) struct Response {
    #[serde(default)]
    model: Option<String>,
    /// Each answer as sent: read per question, so one the catalogue may
    /// leave unrated cannot fail the triage answers beside it.
    answers: HashMap<String, serde_json::Value>,
    #[serde(default)]
    routing: Option<Routing>,
    /// Tokens, when a hosted decision model reports them (laya-serve
    /// doesn't).
    #[serde(default)]
    pub usage: Option<crate::Usage>,
}

#[derive(Deserialize)]
struct Routing {
    /// Hugging Face repository (and subfolder) of the checkpoint that answered.
    repo: Option<String>,
}

#[derive(Deserialize)]
struct WireAnswer {
    choice: Option<String>,
    score: Option<f64>,
    noul: Option<f64>,
    #[serde(default)]
    probabilities: Option<HashMap<String, f64>>,
}

impl Response {
    /// The checkpoint that answered: the routed repository when reported,
    /// else the configured model.
    pub fn model(&self, configured: &str) -> String {
        self.routing
            .as_ref()
            .and_then(|r| r.repo.clone())
            .or_else(|| {
                // laya-serve's top-level `model` is a fixed agent name, not a
                // checkpoint id; only use it when nothing better is there.
                self.model.clone().filter(|m| m != "laya-rl-agent")
            })
            .unwrap_or_else(|| configured.to_owned())
    }

    /// Match every question to its answer. A missing or malformed answer
    /// fails the call, except to a catalogue or a move question
    /// ([`is_tolerant`](super::signals::is_tolerant)), which is then
    /// unanswered (`None`).
    pub fn into_raw(mut self, questions: &[Question]) -> Result<Vec<Option<RawAnswer>>, Error> {
        questions
            .iter()
            .map(|q| {
                let raw = self
                    .answers
                    .remove(&q.id)
                    .ok_or_else(|| {
                        Error::InvalidResponse(format!("no answer for question `{}`", q.id))
                    })
                    .and_then(|value| {
                        let wire: WireAnswer = serde_json::from_value(value).map_err(|_| {
                            Error::InvalidResponse(format!("answer `{}` is malformed", q.id))
                        })?;
                        decode(q, wire)
                    });
                match raw {
                    Ok(raw) => Ok(Some(raw)),
                    Err(error) if super::signals::is_tolerant(&q.id) => {
                        // The error names the question only, never the mail.
                        tracing::debug!(question = %q.id, %error, "tolerated answer dropped");
                        Ok(None)
                    }
                    Err(error) => Err(error),
                }
            })
            .collect()
    }
}

fn decode(question: &Question, wire: WireAnswer) -> Result<RawAnswer, Error> {
    let id = &question.id;
    let missing = |field: &str| Error::InvalidResponse(format!("answer `{id}` has no `{field}`"));
    match &question.kind {
        QuestionKind::Choice { options, .. } => {
            let choice = wire.choice.ok_or_else(|| missing("choice"))?;
            if !options.iter().any(|o| o.label == choice) {
                return Err(Error::InvalidResponse(format!(
                    "answer `{id}` chose an unknown option"
                )));
            }
            let by_label = wire.probabilities.ok_or_else(|| missing("probabilities"))?;
            let probabilities = options
                .iter()
                .map(|o| by_label.get(&o.label).copied())
                .collect::<Option<Vec<f64>>>()
                .ok_or_else(|| missing("probabilities for every option"))?;
            Ok(RawAnswer {
                value: AnswerValue::Choice(choice),
                probabilities: Some(probabilities),
            })
        }
        QuestionKind::Score { levels, .. } => {
            let score = wire.score.ok_or_else(|| missing("score"))?;
            let by_index = wire.probabilities.ok_or_else(|| missing("probabilities"))?;
            let probabilities = (0..levels.len())
                .map(|i| by_index.get(&i.to_string()).copied())
                .collect::<Option<Vec<f64>>>()
                .ok_or_else(|| missing("probabilities for every level"))?;
            Ok(RawAnswer {
                value: AnswerValue::Score(score),
                probabilities: Some(probabilities),
            })
        }
        QuestionKind::Noul { .. } => {
            let p_true = wire.noul.ok_or_else(|| missing("noul"))?;
            if !(0.0..=1.0).contains(&p_true) {
                return Err(Error::InvalidResponse(format!(
                    "answer `{id}` has a probability outside [0, 1]"
                )));
            }
            Ok(RawAnswer {
                value: AnswerValue::Noul(p_true >= 0.5),
                probabilities: Some(vec![1.0 - p_true, p_true]),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn questions() -> Vec<Question> {
        vec![
            Question::choice(
                "category",
                "What is this email about?",
                [("zeta", "Last label alphabetically"), ("alpha", "First")],
            ),
            Question::score("urgency", "How urgent?", ["not urgent", "soon", "today"]),
            Question::noul(
                "asks_user",
                "The sender asks the recipient to do something.",
            ),
        ]
    }

    #[test]
    fn request_keeps_caller_order() {
        let questions = questions();
        let state = DecisionState::from("Subject: Invoice");
        let body = serde_json::to_string(&Request {
            model: "multilingual",
            state: &state,
            questions: &questions,
        })
        .unwrap();
        // Questions and options in the order given, not sorted.
        let category = body.find("\"category\"").unwrap();
        let urgency = body.find("\"urgency\"").unwrap();
        let asks = body.find("\"asks_user\"").unwrap();
        assert!(category < urgency && urgency < asks, "{body}");
        assert!(body.find("\"zeta\"").unwrap() < body.find("\"alpha\"").unwrap());
    }

    #[test]
    fn request_wire_shape() {
        let questions = questions();
        let state = DecisionState::from(json!({"subject": "Invoice"}));
        let body = serde_json::to_value(Request {
            model: "convaiinnovations/laya-multilingual",
            state: &state,
            questions: &questions,
        })
        .unwrap();
        assert_eq!(
            body,
            json!({
                "model": "convaiinnovations/laya-multilingual",
                "state": {"subject": "Invoice"},
                "questions": {
                    "category": {
                        "type": "choice",
                        "instructions": "What is this email about?",
                        "criteria": {"zeta": "Last label alphabetically", "alpha": "First"}
                    },
                    "urgency": {
                        "type": "score",
                        "instructions": "How urgent?",
                        "criteria": ["not urgent", "soon", "today"]
                    },
                    "asks_user": {
                        "type": "noul",
                        "instructions": "The sender asks the recipient to do something."
                    }
                }
            })
        );
    }

    #[test]
    fn decodes_every_type_in_question_order() {
        let response: Response = serde_json::from_value(json!({
            "model": "laya-rl-agent",
            "answers": {
                "asks_user": {"type": "noul", "noul": 0.8, "confidence": 0.8,
                              "answer_confidence": 0.8, "action": {"act_probability": 0.9}},
                "urgency": {"type": "score", "score": 1.2, "legend": {"0": "a", "1": "b", "2": "c"},
                            "probabilities": {"0": 0.2, "1": 0.4, "2": 0.4},
                            "confidence": 0.1, "answer_confidence": 0.4},
                "category": {"type": "choice", "choice": "alpha",
                             "probabilities": {"zeta": 0.3, "alpha": 0.7},
                             "confidence": 0.12, "answer_confidence": 0.7}
            },
            "usage": {"input_tokens": 42, "output_tokens": 0},
            "routing": {"model": "multilingual", "repo": "convaiinnovations/laya/multilingual",
                        "reason": "explicit model='multilingual'", "detection": null, "workflow": null}
        }))
        .unwrap();
        assert_eq!(response.model("cfg"), "convaiinnovations/laya/multilingual");
        let raw: Vec<RawAnswer> = response
            .into_raw(&questions())
            .unwrap()
            .into_iter()
            .map(Option::unwrap)
            .collect();

        assert_eq!(raw[0].value, AnswerValue::Choice("alpha".into()));
        assert_eq!(
            raw[0].probabilities,
            Some(vec![0.3, 0.7]),
            "in option order"
        );
        assert_eq!(raw[1].value, AnswerValue::Score(1.2));
        assert_eq!(raw[1].probabilities, Some(vec![0.2, 0.4, 0.4]));
        assert_eq!(raw[2].value, AnswerValue::Noul(true));
        let p = raw[2].probabilities.as_ref().unwrap();
        assert!((p[0] - 0.2).abs() < 1e-12 && (p[1] - 0.8).abs() < 1e-12);
    }

    #[test]
    fn model_falls_back_to_configured() {
        let response: Response =
            serde_json::from_value(json!({"model": "laya-rl-agent", "answers": {}})).unwrap();
        assert_eq!(
            response.model("convaiinnovations/laya"),
            "convaiinnovations/laya"
        );
    }

    #[test]
    fn rejects_incomplete_answers() {
        let qs = questions();
        let cases = [
            json!({"answers": {}}),
            json!({"answers": {"category": {"choice": "beta", "probabilities": {"zeta": 0.5, "alpha": 0.5}}}}),
            json!({"answers": {"category": {"choice": "alpha", "probabilities": {"alpha": 1.0}}}}),
        ];
        for case in cases {
            let response: Response = serde_json::from_value(case.clone()).unwrap();
            assert!(
                matches!(response.into_raw(&qs[..1]), Err(Error::InvalidResponse(_))),
                "{case}"
            );
        }
        let noul: Response =
            serde_json::from_value(json!({"answers": {"asks_user": {"noul": 1.5}}})).unwrap();
        assert!(noul.into_raw(&qs[2..]).is_err());
        let malformed: Response =
            serde_json::from_value(json!({"answers": {"asks_user": {"noul": "NaN"}}})).unwrap();
        assert!(malformed.into_raw(&qs[2..]).is_err());
    }

    #[test]
    fn a_catalogue_answer_that_does_not_fit_is_no_rating_and_fails_nothing() {
        use crate::decide::signals::actions::question;
        let mut qs = questions();
        for id in [
            question::REMIND,
            question::FORWARD,
            question::PAID_AUTOMATICALLY,
        ] {
            qs.push(Question::noul(id, "A catalogue statement."));
        }
        // Remind missing, Forward out of range, paid not a number at all.
        let response: Response = serde_json::from_value(json!({
            "answers": {
                "category": {"choice": "alpha", "probabilities": {"zeta": 0.3, "alpha": 0.7}},
                "urgency": {"score": 1.0, "probabilities": {"0": 0.2, "1": 0.6, "2": 0.2}},
                "asks_user": {"noul": 0.8},
                "action_forward": {"noul": 1.5},
                "action_paid_automatically": {"noul": "NaN"}
            }
        }))
        .unwrap();
        let raw = response.into_raw(&qs).unwrap();
        assert!(raw[..3].iter().all(Option::is_some), "triage as always");
        assert!(
            raw[3..].iter().all(Option::is_none),
            "no rating, no failure"
        );
    }

    #[test]
    fn a_move_answer_that_does_not_fit_is_nothing_to_record_and_fails_nothing() {
        use crate::decide::signals::moves;
        let mut qs = questions();
        qs.push(moves::next_move_question());
        qs.push(Question::choice(
            moves::question::AWAITED,
            "Who?",
            [("p1", "Ana"), ("none", "Nobody")],
        ));
        // `next_move` missing, `awaited` outside its options.
        let response: Response = serde_json::from_value(json!({
            "answers": {
                "category": {"choice": "alpha", "probabilities": {"zeta": 0.3, "alpha": 0.7}},
                "urgency": {"score": 1.0, "probabilities": {"0": 0.2, "1": 0.6, "2": 0.2}},
                "asks_user": {"noul": 0.8},
                "awaited": {"choice": "p9", "probabilities": {"p1": 0.5, "none": 0.5}}
            }
        }))
        .unwrap();
        let raw = response.into_raw(&qs).unwrap();
        assert!(raw[..3].iter().all(Option::is_some), "triage as always");
        assert!(raw[3..].iter().all(Option::is_none), "nothing to record");
    }
}
