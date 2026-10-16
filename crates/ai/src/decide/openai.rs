//! `decide` over an OpenAI-compatible instruct model (ADR 0009's adapter).
//!
//! The model gets the state and the typed questions in a prompt, and answers
//! with JSON constrained by a strict schema: a `choice` is one of its labels,
//! a `score` one of its level numbers, a `noul` a boolean. Nothing is parsed
//! out of free text.
//!
//! Probabilities come from token log-probabilities when the server returns
//! them (`logprobs: true`, `top_logprobs`). At the token where an answer's
//! value starts, each alternative the model considered is matched to the
//! options it could begin; the chosen path is followed token by token until
//! the options separate. Mass the top alternatives do not cover is dropped and
//! the rest renormalised, so these are estimates, good for gating, not
//! calibrated until refitted. Without log-probabilities an answer has no
//! probabilities and no confidence.

use serde::Deserialize;
use serde_json::{Map, Value, json};

use crate::{
    Error,
    decide::{AnswerValue, DecisionState, Question, QuestionKind, RawAnswer},
    gateway::Message,
};

/// Most alternatives per token OpenAI-compatible servers return.
pub(crate) const TOP_LOGPROBS: u8 = 20;

const SYSTEM_PROMPT: &str = "You answer typed decision questions about a state. Base every \
answer only on the state. Reply with one JSON object that has exactly one field per question \
id: for a choice, one of its option labels; for a score, one of its level numbers; for a \
yes/no statement, true if the statement holds for the state and false otherwise.";

/// The prompt: a fixed system message, then the state and the questions.
pub(crate) fn messages(state: &DecisionState, questions: &[Question]) -> Vec<Message> {
    let state = match state {
        DecisionState::Text(text) => text.clone(),
        DecisionState::Json(value) => {
            serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
        }
    };
    let mut lines = vec![format!("State:\n{state}\n\nQuestions:")];
    for question in questions {
        let id = json_string(&question.id);
        match &question.kind {
            QuestionKind::Choice {
                instructions,
                options,
            } => {
                lines.push(format!("- {id} (choice): {instructions}\n  Options:"));
                for option in options {
                    let label = json_string(&option.label);
                    if option.description.is_empty() {
                        lines.push(format!("  - {label}"));
                    } else {
                        lines.push(format!("  - {label}: {}", option.description));
                    }
                }
            }
            QuestionKind::Score {
                instructions,
                levels,
            } => {
                lines.push(format!(
                    "- {id} (score, answer with the level number): {instructions}\n  Levels:"
                ));
                for (i, level) in levels.iter().enumerate() {
                    lines.push(format!("  {i}: {level}"));
                }
            }
            QuestionKind::Noul { statement } => {
                lines.push(format!("- {id} (true or false): {statement}"));
            }
        }
    }
    let user = lines.join("\n") + "\n";
    vec![Message::system(SYSTEM_PROMPT), Message::user(user)]
}

/// The strict JSON schema of the answer object.
pub(crate) fn schema(questions: &[Question]) -> Value {
    let mut properties = Map::new();
    for question in questions {
        let property = match &question.kind {
            QuestionKind::Choice { options, .. } => json!({
                "type": "string",
                "enum": options.iter().map(|o| o.label.as_str()).collect::<Vec<_>>(),
            }),
            QuestionKind::Score { levels, .. } => json!({
                "type": "integer",
                "enum": (0..levels.len()).collect::<Vec<_>>(),
            }),
            QuestionKind::Noul { .. } => json!({"type": "boolean"}),
        };
        properties.insert(question.id.clone(), property);
    }
    json!({
        "type": "object",
        "properties": properties,
        "required": questions.iter().map(|q| q.id.as_str()).collect::<Vec<_>>(),
        "additionalProperties": false,
    })
}

#[derive(Deserialize)]
pub(crate) struct Response {
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub system_fingerprint: Option<String>,
    choices: Vec<WireChoice>,
    /// Tokens, as OpenAI-compatible servers report them.
    #[serde(default)]
    pub usage: Option<crate::Usage>,
}

#[derive(Deserialize)]
struct WireChoice {
    message: WireMessage,
    #[serde(default)]
    logprobs: Option<WireLogprobs>,
}

#[derive(Deserialize)]
struct WireMessage {
    content: Option<String>,
}

#[derive(Deserialize)]
struct WireLogprobs {
    #[serde(default)]
    content: Option<Vec<WireToken>>,
}

#[derive(Deserialize)]
struct WireToken {
    token: String,
    logprob: f64,
    #[serde(default)]
    bytes: Option<Vec<u8>>,
    #[serde(default)]
    top_logprobs: Vec<WireAlternative>,
}

#[derive(Deserialize)]
struct WireAlternative {
    token: String,
    logprob: f64,
    #[serde(default)]
    bytes: Option<Vec<u8>>,
}

impl Response {
    /// Parse the answer object and, when log-probabilities came back, derive
    /// each answer's option probabilities.
    pub fn into_raw(self, questions: &[Question]) -> Result<Vec<RawAnswer>, Error> {
        let choice = self
            .choices
            .into_iter()
            .next()
            .ok_or_else(|| Error::InvalidResponse("no choices".into()))?;
        let content = choice.message.content.unwrap_or_default();
        let object: Map<String, Value> = serde_json::from_str(&content).map_err(|e| {
            Error::InvalidResponse(format!(
                "the answer is not a JSON object: {}",
                campbooks_kernel::redact::json(&e)
            ))
        })?;
        let tokens = choice
            .logprobs
            .and_then(|l| l.content)
            .and_then(|t| Tokens::new(t, content.as_bytes()));

        questions
            .iter()
            .map(|q| {
                let value = object.get(&q.id).ok_or_else(|| {
                    Error::InvalidResponse(format!("no answer for question `{}`", q.id))
                })?;
                let value = parse_value(q, value)?;
                let probabilities = tokens.as_ref().and_then(|t| {
                    let start = value_start(content.as_bytes(), &q.id)?;
                    t.option_probabilities(start, &option_texts(q))
                });
                Ok(RawAnswer {
                    value,
                    probabilities,
                })
            })
            .collect()
    }
}

fn parse_value(question: &Question, value: &Value) -> Result<AnswerValue, Error> {
    let invalid = || {
        Error::InvalidResponse(format!(
            "answer `{}` does not fit its question",
            question.id
        ))
    };
    match &question.kind {
        QuestionKind::Choice { options, .. } => {
            let label = value.as_str().ok_or_else(invalid)?;
            if !options.iter().any(|o| o.label == label) {
                return Err(invalid());
            }
            Ok(AnswerValue::Choice(label.to_owned()))
        }
        QuestionKind::Score { levels, .. } => {
            let level = value
                .as_u64()
                .and_then(|l| u32::try_from(l).ok())
                .filter(|l| (*l as usize) < levels.len())
                .ok_or_else(invalid)?;
            Ok(AnswerValue::Score(f64::from(level)))
        }
        QuestionKind::Noul { .. } => Ok(AnswerValue::Noul(value.as_bool().ok_or_else(invalid)?)),
    }
}

/// Each option as the model would write it in JSON, in option order (`noul`:
/// `false` then `true`).
fn option_texts(question: &Question) -> Vec<Vec<u8>> {
    match &question.kind {
        QuestionKind::Choice { options, .. } => options
            .iter()
            .map(|o| json_string(&o.label).into_bytes())
            .collect(),
        QuestionKind::Score { levels, .. } => (0..levels.len())
            .map(|i| i.to_string().into_bytes())
            .collect(),
        QuestionKind::Noul { .. } => vec![b"false".to_vec(), b"true".to_vec()],
    }
}

fn json_string(s: &str) -> String {
    Value::String(s.to_owned()).to_string()
}

/// Byte offset where the value of top-level key `id` starts in `content`.
fn value_start(content: &[u8], id: &str) -> Option<usize> {
    let key = json_string(id).into_bytes();
    let mut from = 0;
    while let Some(found) = find(&content[from..], &key) {
        let mut i = from + found + key.len();
        while content.get(i).is_some_and(u8::is_ascii_whitespace) {
            i += 1;
        }
        if content.get(i) == Some(&b':') {
            i += 1;
            while content.get(i).is_some_and(u8::is_ascii_whitespace) {
                i += 1;
            }
            return (i < content.len()).then_some(i);
        }
        from += found + 1;
    }
    None
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// The generated tokens, with byte offsets into the content.
struct Tokens {
    list: Vec<Token>,
    starts: Vec<usize>,
    content: Vec<u8>,
}

struct Token {
    text: Vec<u8>,
    p: f64,
    alternatives: Vec<(Vec<u8>, f64)>,
}

enum Fit {
    /// The token is a prefix of the option's remaining text.
    Continues,
    /// The token completes the option.
    Completes,
    /// The token rules the option out.
    No,
}

impl Tokens {
    /// `None` when the tokens do not spell out the content (some servers
    /// report tokens that do not round-trip).
    fn new(wire: Vec<WireToken>, content: &[u8]) -> Option<Self> {
        let bytes =
            |token: String, bytes: Option<Vec<u8>>| bytes.unwrap_or_else(|| token.into_bytes());
        let mut tokens = Vec::with_capacity(wire.len());
        let mut starts = Vec::with_capacity(wire.len());
        let mut offset = 0;
        for t in wire {
            let text = bytes(t.token, t.bytes);
            starts.push(offset);
            offset += text.len();
            let alternatives = t
                .top_logprobs
                .into_iter()
                .map(|a| (bytes(a.token, a.bytes), a.logprob.exp()))
                .filter(|(alt, _)| *alt != text)
                .collect();
            tokens.push(Token {
                text,
                p: t.logprob.exp(),
                alternatives,
            });
        }
        let spelled: Vec<u8> = tokens.iter().flat_map(|t| t.text.iter().copied()).collect();
        (spelled == content).then(|| Self {
            list: tokens,
            starts,
            content: content.to_vec(),
        })
    }

    /// Probability of each option for the value starting at byte `start`.
    fn option_probabilities(&self, start: usize, options: &[Vec<u8>]) -> Option<Vec<f64>> {
        let first = self
            .starts
            .iter()
            .zip(&self.list)
            .position(|(s, t)| *s <= start && start < s + t.text.len())?;
        let mut mass = vec![0.0; options.len()];
        // (option index, what is left of it to match)
        let mut alive: Vec<(usize, &[u8])> = options
            .iter()
            .enumerate()
            .map(|(i, o)| (i, o.as_slice()))
            .collect();
        let mut prefix = 1.0;

        for (index, token) in self.list.iter().enumerate().skip(first) {
            let token_start = self.starts[index];
            let pre_len = if index == first {
                start - token_start
            } else {
                0
            };
            let pre = &token.text[..pre_len];
            let chosen = &token.text[pre_len..];
            let after_chosen = self.content.get(token_start + token.text.len()).copied();

            let mut on_path = Vec::new();
            let mut next_alive = Vec::new();
            for &(option, rest) in &alive {
                match fit(rest, chosen, after_chosen, &options[option]) {
                    Fit::Continues => {
                        on_path.push(option);
                        next_alive.push((option, &rest[chosen.len()..]));
                    }
                    Fit::Completes => {
                        on_path.push(option);
                        mass[option] += prefix * token.p;
                    }
                    Fit::No => {}
                }
            }

            // Alternatives the model weighed here, credited to the options
            // they begin (shared evenly when they begin several).
            for (alt, p) in &token.alternatives {
                let Some(r) = alt.strip_prefix(pre).filter(|r| !r.is_empty()) else {
                    continue;
                };
                let next = None; // what would follow an alternative is unknown
                let begun: Vec<usize> = alive
                    .iter()
                    .filter(|(option, rest)| {
                        !on_path.contains(option)
                            && !matches!(fit(rest, r, next, &options[*option]), Fit::No)
                    })
                    .map(|(option, _)| *option)
                    .collect();
                if !begun.is_empty() {
                    let share = prefix * p / begun.iter().map(|_| 1.0).sum::<f64>();
                    for option in begun {
                        mass[option] += share;
                    }
                }
            }

            prefix *= token.p;
            alive = next_alive;
            if alive.is_empty() {
                break;
            }
        }
        // Ran out of tokens mid-option: what is left of the path is theirs.
        for (option, _) in alive {
            mass[option] += prefix;
        }

        let total: f64 = mass.iter().sum();
        (total.is_finite() && total > 0.0).then(|| mass.iter().map(|m| m / total).collect())
    }
}

/// How token text `r` relates to `rest`, the unmatched end of `option`.
/// `after` is the byte that follows the token in the content, if known.
fn fit(rest: &[u8], r: &[u8], after: Option<u8>, option: &[u8]) -> Fit {
    // Strings end with their closing quote; numbers and literals end where
    // the next character is not part of them.
    let self_terminated = option.last() == Some(&b'"');
    let continues_word = |b: u8| b.is_ascii_alphanumeric() || b == b'.';
    if r.len() < rest.len() {
        return if rest.starts_with(r) {
            Fit::Continues
        } else {
            Fit::No
        };
    }
    if !r.starts_with(rest) {
        return Fit::No;
    }
    let next = r.get(rest.len()).copied().or(after);
    if !self_terminated && next.is_some_and(continues_word) {
        // "1" followed by "0": not the option "1" — unless the token ends
        // exactly here and a later token can still end it.
        return if r.len() == rest.len() {
            Fit::Continues
        } else {
            Fit::No
        };
    }
    Fit::Completes
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tok(text: &str, p: f64, alternatives: &[(&str, f64)]) -> WireToken {
        WireToken {
            token: text.into(),
            logprob: p.ln(),
            bytes: None,
            top_logprobs: alternatives
                .iter()
                .map(|(t, p)| WireAlternative {
                    token: (*t).into(),
                    logprob: p.ln(),
                    bytes: None,
                })
                .collect(),
        }
    }

    fn probs(tokens: Vec<WireToken>, id: &str, question: &Question) -> Option<Vec<f64>> {
        let content: String = tokens.iter().map(|t| t.token.clone()).collect();
        let t = Tokens::new(tokens, content.as_bytes())?;
        t.option_probabilities(
            value_start(content.as_bytes(), id)?,
            &option_texts(question),
        )
    }

    fn close(a: &[f64], b: &[f64]) -> bool {
        a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-9)
    }

    #[test]
    fn noul_from_the_boolean_token() {
        let q = Question::noul("asks", "It asks something.");
        let tokens = vec![
            tok("{\"", 1.0, &[]),
            tok("asks", 1.0, &[]),
            tok("\":", 1.0, &[]),
            tok(" true", 0.75, &[(" false", 0.2), (" maybe", 0.05)]),
            tok("}", 1.0, &[]),
        ];
        let p = probs(tokens, "asks", &q).unwrap();
        // 0.75 vs 0.2; the unrelated alternative is dropped, then renormalised.
        assert!(close(&p, &[0.2 / 0.95, 0.75 / 0.95]), "{p:?}");
    }

    #[test]
    fn choice_follows_shared_prefixes_until_options_separate() {
        let q = Question::choice(
            "cat",
            "Which?",
            [("billing", ""), ("bills", ""), ("support", "")],
        );
        // "billing" chosen as `"` `bill` `ing"`; at `bill` the model also
        // weighed `supp`; at `ing` it weighed `s` (→ "bills").
        let tokens = vec![
            tok("{\"cat\": ", 1.0, &[]),
            tok("\"", 1.0, &[]),
            tok("bill", 0.8, &[("supp", 0.2)]),
            tok("ing\"", 0.5, &[("s\"", 0.5)]),
            tok("}", 1.0, &[]),
        ];
        let p = probs(tokens, "cat", &q).unwrap();
        // billing 0.8·0.5 = 0.4, bills 0.8·0.5 = 0.4, support 0.2
        assert!(close(&p, &[0.4, 0.4, 0.2]), "{p:?}");
    }

    #[test]
    fn choice_with_the_quote_inside_a_larger_token() {
        let q = Question::choice("cat", "Which?", [("a", ""), ("b", "")]);
        let tokens = vec![
            tok("{\"cat\":\"", 1.0, &[("{\"cat\": \"", 0.1)]),
            tok("a", 0.6, &[("b", 0.3)]),
            tok("\"}", 1.0, &[]),
        ];
        let p = probs(tokens, "cat", &q).unwrap();
        assert!(close(&p, &[2.0 / 3.0, 1.0 / 3.0]), "{p:?}");
    }

    #[test]
    fn score_numbers_need_a_terminator() {
        // 12 levels: "1" must not swallow the mass of "10".
        let q = Question::score("s", "How?", (0..12).map(|i| format!("level {i}")));
        let tokens = vec![
            tok("{\"s\": ", 1.0, &[]),
            tok("1", 0.9, &[("2", 0.1)]),
            tok("0", 0.5, &[("1", 0.25), ("}", 0.25)]),
            tok("}", 1.0, &[]),
        ];
        let p = probs(tokens, "s", &q).unwrap();
        // 10: 0.9·0.5, 11: 0.9·0.25, 1: 0.9·0.25 (the model weighed ending
        // after "1"), 2: 0.1. Nothing else.
        let mut expected = vec![0.0; 12];
        expected[10] = 0.45;
        expected[11] = 0.225;
        expected[1] = 0.225;
        expected[2] = 0.1;
        assert!(close(&p, &expected), "{p:?}");
    }

    #[test]
    fn tokens_that_do_not_spell_the_content_give_no_probabilities() {
        let content = b"{\"a\": true}";
        let tokens = vec![tok("{\"a\": ", 1.0, &[]), tok("tru", 1.0, &[])];
        assert!(Tokens::new(tokens, content).is_none());
    }

    #[test]
    fn utf8_labels_use_token_bytes() {
        let q = Question::choice("c", "Which?", [("é", ""), ("e", "")]);
        let content = "{\"c\":\"é\"}";
        // "é" split across two byte-level tokens, as BPE vocabularies do.
        let wire = vec![
            tok("{\"c\":\"", 1.0, &[]),
            WireToken {
                token: "\\xc3".into(),
                logprob: 0.7_f64.ln(),
                bytes: Some(vec![0xc3]),
                top_logprobs: vec![WireAlternative {
                    token: "e".into(),
                    logprob: 0.3_f64.ln(),
                    bytes: Some(vec![b'e']),
                }],
            },
            WireToken {
                token: "\\xa9".into(),
                logprob: 0.0,
                bytes: Some(vec![0xa9]),
                top_logprobs: vec![],
            },
            tok("\"}", 1.0, &[]),
        ];
        let t = Tokens::new(wire, content.as_bytes()).unwrap();
        let p = t
            .option_probabilities(
                value_start(content.as_bytes(), "c").unwrap(),
                &option_texts(&q),
            )
            .unwrap();
        assert!(close(&p, &[0.7, 0.3]), "{p:?}");
    }

    #[test]
    fn value_start_skips_whitespace_and_values_that_look_like_keys() {
        let content = br#"{"a": "b", "b" :  1}"#;
        assert_eq!(value_start(content, "a"), Some(6));
        assert_eq!(content[value_start(content, "b").unwrap()], b'1');
        assert_eq!(value_start(content, "c"), None);
    }

    #[test]
    fn schema_is_strict_and_typed() {
        let questions = [
            Question::choice("c", "Which?", [("x", "X"), ("y", "Y")]),
            Question::score("s", "How?", ["low", "high"]),
            Question::noul("n", "True?"),
        ];
        assert_eq!(
            schema(&questions),
            json!({
                "type": "object",
                "properties": {
                    "c": {"type": "string", "enum": ["x", "y"]},
                    "s": {"type": "integer", "enum": [0, 1]},
                    "n": {"type": "boolean"}
                },
                "required": ["c", "s", "n"],
                "additionalProperties": false
            })
        );
    }

    #[test]
    fn prompt_lists_every_question() {
        let questions = [
            Question::choice("c", "Which?", [("x", "the X"), ("y", "")]),
            Question::score("s", "How?", ["low", "high"]),
            Question::noul("n", "It is spam."),
        ];
        let messages = messages(&DecisionState::from("Hello"), &questions);
        let crate::gateway::Content::Text(user) = &messages[1].content else {
            panic!("text prompt")
        };
        for needle in [
            "State:\nHello",
            "- \"c\" (choice): Which?",
            "  - \"x\": the X",
            "  - \"y\"\n",
            "  1: high",
            "- \"n\" (true or false): It is spam.",
        ] {
            assert!(user.contains(needle), "missing {needle:?} in\n{user}");
        }
    }

    #[test]
    fn rejects_answers_outside_the_question() {
        let q = [
            Question::choice("c", "Which?", [("x", ""), ("y", "")]),
            Question::score("s", "How?", ["a", "b"]),
            Question::noul("n", "?"),
        ];
        for (question, value) in [
            (&q[0], json!("z")),
            (&q[0], json!(1)),
            (&q[1], json!(2)),
            (&q[1], json!(-1)),
            (&q[1], json!("1")),
            (&q[2], json!("yes")),
        ] {
            assert!(parse_value(question, &value).is_err(), "{value}");
        }
        assert_eq!(
            parse_value(&q[1], &json!(1)).unwrap(),
            AnswerValue::Score(1.0)
        );
    }
}
