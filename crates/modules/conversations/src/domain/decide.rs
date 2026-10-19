//! What the decide pass reads about a message, and where its answers come
//! from.
//!
//! The strategies, the triage questions and the facts live in
//! `campbooks_ai::decide` (`strategy`, `signals::email`, `signals::facts`),
//! measured by `cargo xtask eval decide`; this module only builds the
//! [`Envelope`] from a stored message and records the answers.

use std::collections::BTreeMap;

use campbooks_ai::decide::signals::email::{Attachment, Envelope, Party};

use crate::{ingest::Headers, model::Address};

/// The version of the envelope this module builds, recorded with every
/// answer so a labelled example can be rebuilt as it was seen. Version 1
/// had no triage context; version `1 + n` is the email after context
/// version `n` ([`campbooks_ai::decide::signals::inbox::CONTEXT_VERSION`]),
/// so a change in what the model is told shows on every stored answer.
/// Version 3 (context 2) is the first of the facts (ADR 0018): rules
/// answers of an older version came from the retired content rules.
pub const STATE_VERSION: i16 = 1 + campbooks_ai::decide::signals::inbox::CONTEXT_VERSION;

/// Bytes of the text body read into an envelope (the model reads its first
/// 1,500 characters; the facts read none of it).
pub const ENVELOPE_BODY_BYTES: usize = 16 * 1024;

/// Bytes of the HTML body read for an envelope when the text body has no
/// words: the words read from it are cut to [`ENVELOPE_BODY_BYTES`].
pub const ENVELOPE_HTML_BYTES: usize = 256 * 1024;

/// What an envelope reads of a message's parts ([`super::blobs::words`]).
pub const ENVELOPE_BUDGET: super::blobs::Budget = super::blobs::Budget {
    text: ENVELOPE_BODY_BYTES,
    html: ENVELOPE_HTML_BYTES,
};

/// The purpose the model pass is traced and evaluated under.
pub const PURPOSE: &str = "conversations.triage";

/// Which kind of strategy gave an answer, as recorded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// The facts: headers and addresses, which only say "no" (ADR 0018).
    /// Answers stored before state version 3 came from the retired content
    /// rules and no longer act.
    Rules,
    /// The `decide` model.
    Model,
    /// A rule a member taught (`campbooks_ai::decide::taught`).
    Taught,
}

impl Source {
    /// The database name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Rules => "rules",
            Self::Model => "model",
            Self::Taught => "taught",
        }
    }
}

/// The parts of a message an envelope is built from.
#[derive(Debug, Clone, Copy)]
pub struct MessageParts<'a> {
    /// The sender.
    pub from: &'a Address,
    /// Recipients.
    pub to: &'a [Address],
    /// Copied recipients.
    pub cc: &'a [Address],
    /// `Reply-To`.
    pub reply_to: &'a [Address],
    /// The subject.
    pub subject: &'a str,
    /// The headers subset.
    pub headers: &'a Headers,
    /// Attachment names and media types.
    pub attachments: &'a [(String, String)],
    /// The start of the text body.
    pub body: &'a str,
}

/// The envelope strategies read about a message.
pub fn envelope(parts: MessageParts<'_>) -> Envelope {
    let mut headers = BTreeMap::new();
    for (name, value) in [
        ("List-Id", &parts.headers.list_id),
        ("List-Unsubscribe", &parts.headers.list_unsubscribe),
        ("Precedence", &parts.headers.precedence),
        ("Auto-Submitted", &parts.headers.auto_submitted),
        (
            "Authentication-Results",
            &parts.headers.authentication_results,
        ),
    ] {
        if let Some(value) = value {
            headers.insert(name.to_owned(), value.clone());
        }
    }
    if !parts.reply_to.is_empty() {
        let reply_to: Vec<&str> = parts.reply_to.iter().map(|a| a.address.as_str()).collect();
        headers.insert("Reply-To".to_owned(), reply_to.join(", "));
    }
    Envelope {
        from: party(parts.from),
        to: parts.to.iter().map(party).collect(),
        cc: parts.cc.iter().map(party).collect(),
        subject: parts.subject.to_owned(),
        headers,
        attachments: parts
            .attachments
            .iter()
            .map(|(name, content_type)| Attachment {
                name: name.clone(),
                content_type: content_type.clone(),
            })
            .collect(),
        body: parts.body.to_owned(),
        // The decide pass adds whose inbox it is; the facts do not read it.
        recipient: None,
    }
}

fn party(address: &Address) -> Party {
    Party {
        name: address.name.clone().unwrap_or_default(),
        address: address.address.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn envelopes_carry_the_headers_the_facts_read() {
        let from = Address {
            name: Some("The Paper Kite".into()),
            address: "hello@letters.paperkite.example".into(),
        };
        let reply_to = [Address {
            name: None,
            address: "editor@paperkite.example".into(),
        }];
        let headers = Headers {
            list_unsubscribe: Some("<mailto:u@paperkite.example>".into()),
            ..Headers::default()
        };
        let envelope = envelope(MessageParts {
            from: &from,
            to: &[],
            cc: &[],
            reply_to: &reply_to,
            subject: "Issue 142",
            headers: &headers,
            attachments: &[("issue.pdf".into(), "application/pdf".into())],
            body: "This week…",
        });
        assert_eq!(
            envelope.header("list-unsubscribe"),
            Some("<mailto:u@paperkite.example>")
        );
        assert_eq!(
            envelope.header("reply-to"),
            Some("editor@paperkite.example")
        );
        assert_eq!(envelope.from.name, "The Paper Kite");
        assert_eq!(envelope.attachments[0].name, "issue.pdf");
    }
}
