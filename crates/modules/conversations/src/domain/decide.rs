//! The seam to the decide strategies: an [`Envelope`] built from a stored
//! message goes in, one optional [`Verdict`] per question comes out.
//!
//! The sorting rules and the strategies themselves (rules, k-NN, model,
//! cascade) belong to `campbooks_ai::decide`, where campbooks-cloud#59 builds
//! and measures them. Until that lands, this module holds a trait of the
//! same shape, [`NoRules`] (abstains on everything: no rules are forked
//! here) and [`ModelStrategy`] over [`Gateway::decide`]. When #59 merges,
//! these are replaced by its types; nothing else in the module changes.

use std::{fmt, future::Future, pin::Pin};

use campbooks_ai::{
    Error, Gateway,
    decide::{Answer, DecisionContext, DecisionState, Question},
};

use crate::{
    ingest::{Headers, ProviderCategory},
    model::Address,
};

/// A boxed future, for the object-safe [`Strategy`].
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// The version of [`Envelope`]'s content and of
/// [`decision_state`](Envelope::decision_state), recorded with every answer
/// so a labelled example can be rebuilt as it was seen.
pub const STATE_VERSION: i16 = 1;

/// Characters of the text body an envelope keeps.
pub const BODY_CHARS: usize = 1_500;

/// The purpose under which the model pass is traced and evaluated.
pub const PURPOSE: &str = "conversations.message_pass";

/// A compact, deterministic view of one message: what every strategy reads.
/// Its `Debug` output carries no content.
#[derive(Clone, PartialEq, Eq, Default)]
pub struct Envelope {
    /// The inbox's own address.
    pub inbox: String,
    /// The sender.
    pub from: Option<Address>,
    /// `Reply-To`.
    pub reply_to: Vec<Address>,
    /// Recipients.
    pub to: Vec<Address>,
    /// How many people were copied.
    pub cc_count: usize,
    /// The subject.
    pub subject: String,
    /// The headers subset.
    pub headers: Headers,
    /// The provider's own categories.
    pub categories: Vec<ProviderCategory>,
    /// Attachment names and media types.
    pub attachments: Vec<(String, String)>,
    /// The first [`BODY_CHARS`] characters of the text body.
    pub body: String,
    /// The language of the inbox's member, when known.
    pub lang_hint: Option<String>,
}

impl fmt::Debug for Envelope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Envelope")
            .field("attachments", &self.attachments.len())
            .field("body_chars", &self.body.chars().count())
            .finish_non_exhaustive()
    }
}

impl Envelope {
    /// The text a model reads: the headers that matter, sender, subject,
    /// attachments and the start of the body.
    pub fn decision_state(&self) -> DecisionState {
        let mut lines = Vec::new();
        if let Some(from) = &self.from {
            lines.push(format!("From: {}", show(from)));
        }
        if !self.reply_to.is_empty() {
            let reply_to: Vec<String> = self.reply_to.iter().map(show).collect();
            lines.push(format!("Reply-To: {}", reply_to.join(", ")));
        }
        lines.push(format!("To: {}", self.inbox));
        if self.to.len() > 1 || self.cc_count > 0 {
            lines.push(format!(
                "Other recipients: {}",
                self.to.len().saturating_sub(1) + self.cc_count
            ));
        }
        for (name, value) in [
            ("List-Id", &self.headers.list_id),
            ("List-Unsubscribe", &self.headers.list_unsubscribe),
            ("Precedence", &self.headers.precedence),
            ("Auto-Submitted", &self.headers.auto_submitted),
        ] {
            if value.is_some() {
                lines.push(format!("{name}: present"));
            }
        }
        if !self.attachments.is_empty() {
            let names: Vec<&str> = self.attachments.iter().map(|(n, _)| n.as_str()).collect();
            lines.push(format!("Attachments: {}", names.join(", ")));
        }
        lines.push(format!("Subject: {}", self.subject));
        lines.push(String::new());
        lines.push(self.body.clone());
        DecisionState::Text(lines.join("\n"))
    }
}

fn show(address: &Address) -> String {
    match &address.name {
        Some(name) => format!("{name} <{}>", address.address),
        None => address.address.clone(),
    }
}

/// Which kind of strategy gave an answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// Pure functions of the envelope.
    Rules,
    /// A model through the `decide` role.
    Model,
}

impl Source {
    /// The database name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Rules => "rules",
            Self::Model => "model",
        }
    }
}

/// One strategy's answer to one question, and why.
#[derive(Debug, Clone, PartialEq)]
pub struct Verdict {
    /// The answer, with its probabilities and confidence.
    pub answer: Answer,
    /// Why, in words a member can read; rules always say.
    pub explanation: Option<String>,
}

/// A way of answering the questions about an envelope.
pub trait Strategy: Send + Sync + 'static {
    /// The strategy's name, recorded with each answer.
    fn name(&self) -> &str;
    /// What kind of strategy it is.
    fn source(&self) -> Source;
    /// One entry per question, in order; `None` when it abstains.
    fn decide<'a>(
        &'a self,
        envelope: &'a Envelope,
        questions: &'a [Question],
    ) -> BoxFuture<'a, Result<Vec<Option<Verdict>>, Error>>;
}

/// The rules strategy until campbooks-cloud#59 lands its signals: abstains
/// on every question, so every conversation goes to Primary.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoRules;

impl Strategy for NoRules {
    fn name(&self) -> &'static str {
        "rules"
    }

    fn source(&self) -> Source {
        Source::Rules
    }

    fn decide<'a>(
        &'a self,
        _envelope: &'a Envelope,
        questions: &'a [Question],
    ) -> BoxFuture<'a, Result<Vec<Option<Verdict>>, Error>> {
        Box::pin(async move { Ok(vec![None; questions.len()]) })
    }
}

/// Every question in one call to the `decide` role (Laya, or an instruct
/// model), about [`Envelope::decision_state`].
#[derive(Debug, Clone)]
pub struct ModelStrategy {
    gateway: Gateway,
}

impl ModelStrategy {
    /// A strategy asking `gateway`'s `decide` role.
    pub fn new(gateway: Gateway) -> Self {
        Self { gateway }
    }
}

impl Strategy for ModelStrategy {
    fn name(&self) -> &'static str {
        "model"
    }

    fn source(&self) -> Source {
        Source::Model
    }

    fn decide<'a>(
        &'a self,
        envelope: &'a Envelope,
        questions: &'a [Question],
    ) -> BoxFuture<'a, Result<Vec<Option<Verdict>>, Error>> {
        Box::pin(async move {
            let answers = self
                .gateway
                .decide(
                    &DecisionContext::new(PURPOSE),
                    &envelope.decision_state(),
                    questions,
                )
                .await?;
            Ok(questions
                .iter()
                .map(|q| {
                    answers.get(&q.id).cloned().map(|answer| Verdict {
                        answer,
                        explanation: None,
                    })
                })
                .collect())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_state_names_signals_without_their_values() {
        let envelope = Envelope {
            inbox: "me@example.com".into(),
            from: Some(Address {
                name: Some("Shop".into()),
                address: "news@shop.example".into(),
            }),
            subject: "Autumn sale".into(),
            headers: Headers {
                list_unsubscribe: Some("<mailto:u@shop.example>".into()),
                ..Headers::default()
            },
            attachments: vec![("catalogue.pdf".into(), "application/pdf".into())],
            body: "Everything half price.".into(),
            ..Envelope::default()
        };
        let DecisionState::Text(text) = envelope.decision_state() else {
            panic!("text state");
        };
        assert!(text.contains("From: Shop <news@shop.example>"));
        assert!(text.contains("List-Unsubscribe: present"));
        assert!(!text.contains("mailto:"));
        assert!(text.contains("Attachments: catalogue.pdf"));
        assert!(text.ends_with("Everything half price."));
        assert!(!format!("{envelope:?}").contains("half price"));
    }

    #[tokio::test]
    async fn no_rules_abstains_on_everything() {
        let questions = crate::domain::questions::all();
        let verdicts = NoRules
            .decide(&Envelope::default(), &questions)
            .await
            .unwrap();
        assert_eq!(verdicts.len(), questions.len());
        assert!(verdicts.iter().all(Option::is_none));
    }
}
