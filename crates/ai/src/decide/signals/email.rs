//! Email as triage reads it: the [`Envelope`], the triage questions
//! ([`triage_questions`]) and what a `decide` model reads of an email
//! ([`Envelope::decision_state`]).
//!
//! Nothing here decides what an email means (ADR 0018): that is the
//! model's job. The deterministic part of triage, which reads structured
//! facts only and may only say "no", is [`super::facts`].

use std::{
    collections::BTreeMap,
    fmt::{self, Write as _},
    sync::LazyLock,
};

use regex::Regex;
use serde::Deserialize;

pub use super::facts::{is_machine_sender, is_public_suffix, registrable_domain};
pub use super::inbox::Recipient;
use crate::decide::{DecisionState, Question};

/// Question ids of the email triage pass (`conversations.triage`).
pub mod triage {
    /// Which stream (`choice`: primary, updates, receipts, newsletters).
    pub const STREAM: &str = "stream";
    /// The sender expects a written reply (`noul`).
    pub const NEEDS_REPLY: &str = "needs_reply";
    /// The sender asks the recipient to do something (`noul`).
    pub const ASKS_ACTION: &str = "asks_action";
    /// There is a time by which the recipient must act (`noul`).
    pub const HAS_DEADLINE: &str = "has_deadline";
    /// How soon (`score`: none, within days, today).
    pub const URGENCY: &str = "urgency";
    /// Phishing, scam or impersonation (`noul`).
    pub const SUSPICIOUS: &str = "suspicious";
    /// What the email says about money (`choice`: [`MONEY_OPTIONS`]).
    pub const MONEY: &str = "money";
    /// The built-in streams, in option order.
    pub const STREAMS: [&str; 4] = ["primary", "updates", "receipts", "newsletters"];
    /// It asks for a payment the recipient still owes.
    pub const OWED: &str = "owed";
    /// It confirms a payment already made (a receipt, a payment confirmation).
    pub const PAID: &str = "paid";
    /// Neither.
    pub const NEITHER: &str = "neither";
    /// The money question's options, in option order.
    pub const MONEY_OPTIONS: [&str; 3] = [OWED, PAID, NEITHER];
}

/// The triage questions, worded for a `decide` model. The labels and their
/// meaning follow `crates/ai/evals/decide/mail-triage/LABELLING.md`. `noul`
/// questions are yes/no questions, the form Laya documents; a declarative
/// statement got the same answer as its negation (campbooks-cloud#128).
/// Changing a wording changes what the model reads: bump
/// [`super::inbox::CONTEXT_VERSION`] with it.
pub fn triage_questions() -> Vec<Question> {
    vec![
        Question::choice(
            triage::STREAM,
            "Which stream does this email belong to?",
            [
                (
                    "primary",
                    "A person writing to the recipient personally (a client, colleague, \
                     supplier's staff, relative), including a person sending an invoice by hand",
                ),
                (
                    "updates",
                    "An automated notification that is not about money: shipping, account or \
                     security alerts, appointments, password resets, auto-replies",
                ),
                (
                    "receipts",
                    "Automated money mail from a system: receipts, order confirmations with an \
                     amount, invoices or bills from a service, payment confirmations or failures",
                ),
                (
                    "newsletters",
                    "Content or marketing sent to a list: newsletters, promotions, digests, \
                     announcements",
                ),
            ],
        ),
        Question::noul(
            triage::NEEDS_REPLY,
            "Does the sender expect the recipient to write back?",
        ),
        Question::noul(
            triage::ASKS_ACTION,
            "Does the sender ask the recipient to do something, such as pay, sign, send, review, \
             confirm or renew? Marketing calls to action and \"if this wasn't you\" notes do not \
             count.",
        ),
        Question::noul(
            triage::HAS_DEADLINE,
            "Does the email give a date or time by which the recipient must act, or after which \
             something affecting them lapses? A meeting time alone or a sale ending does not \
             count.",
        ),
        Question::score(
            triage::URGENCY,
            "How soon does the recipient need to act?",
            [
                "No action, or no time pressure",
                "Within the next days",
                "Today: within 24 hours, overdue, or urgent",
            ],
        ),
        Question::noul(
            triage::SUSPICIOUS,
            "Is this email phishing, a scam or an impersonation, such as a look-alike sender \
             domain, credentials or payment asked for through a link, an urgent transfer or gift \
             cards?",
        ),
        Question::choice(
            triage::MONEY,
            "What does this email say about money the recipient pays?",
            [
                (
                    triage::OWED,
                    "It asks for a payment the recipient still owes: an invoice or bill to pay, \
                     a payment due or overdue, an amount that will be debited",
                ),
                (
                    triage::PAID,
                    "It confirms a payment already made: a receipt, an order or payment \
                     confirmation, a refund",
                ),
                (
                    triage::NEITHER,
                    "Neither: it asks for no payment and confirms none",
                ),
            ],
        ),
    ]
}

/// A sender or recipient.
#[derive(Clone, Default, PartialEq, Eq, Deserialize)]
pub struct Party {
    /// Display name; may be empty.
    #[serde(default)]
    pub name: String,
    /// Email address.
    pub address: String,
}

/// An attachment's name and media type (never its content).
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct Attachment {
    /// File name.
    pub name: String,
    /// Media type (`application/pdf`).
    pub content_type: String,
}

/// What triage reads of an email: the envelope, the headers that matter and
/// the plain-text body. Its `Debug` output never includes content.
#[derive(Clone, Default, PartialEq, Deserialize)]
pub struct Envelope {
    /// Sender.
    pub from: Party,
    /// Recipients.
    #[serde(default)]
    pub to: Vec<Party>,
    /// Copied recipients.
    #[serde(default)]
    pub cc: Vec<Party>,
    /// Subject.
    #[serde(default)]
    pub subject: String,
    /// Headers the provider exposed (`List-Unsubscribe`, `Auto-Submitted`,
    /// `Reply-To`, …). Missing means not given, not "no".
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
    /// Attachments.
    #[serde(default)]
    pub attachments: Vec<Attachment>,
    /// Plain-text body, quoted history included.
    #[serde(default)]
    pub body: String,
    /// Whose inbox it arrived in, when known: what a `decide` model is told
    /// about the recipient ([`super::inbox`]).
    #[serde(default)]
    pub recipient: Option<Recipient>,
}

impl fmt::Debug for Party {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Party(..)")
    }
}

impl fmt::Debug for Envelope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Envelope({} headers, {} attachments, {} body chars)",
            self.headers.len(),
            self.attachments.len(),
            self.body.chars().count()
        )
    }
}

/// Headers a `decide` model reads: the list and automation headers and
/// `Reply-To`.
const STATE_HEADERS: [&str; 5] = [
    "list-unsubscribe",
    "list-id",
    "precedence",
    "auto-submitted",
    "reply-to",
];
/// Body characters a `decide` state keeps.
const STATE_BODY_CHARS: usize = 1500;

impl Envelope {
    /// A header's value, by case-insensitive name.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    /// The compact state a `decide` model reads (ADR 0009): the triage
    /// context ([`super::inbox::preamble`]: what Campbooks is, who the
    /// recipient is, the facts as hints), then the email: sender,
    /// recipients, subject, the list and automation headers present, the
    /// attachment list and the first part of the body.
    pub fn decision_state(&self) -> DecisionState {
        let mut out = super::inbox::preamble(self);
        let _ = writeln!(out, "From: {}", party(&self.from));
        let to: Vec<String> = self.to.iter().chain(&self.cc).map(party).collect();
        if !to.is_empty() {
            let _ = writeln!(out, "To: {}", to.join(", "));
        }
        let _ = writeln!(out, "Subject: {}", self.subject);
        for name in STATE_HEADERS {
            if let Some(value) = self.header(name) {
                let _ = writeln!(out, "{name}: {}", truncate(value, 120));
            }
        }
        if !self.attachments.is_empty() {
            let list: Vec<String> = self
                .attachments
                .iter()
                .map(|a| format!("{} ({})", a.name, a.content_type))
                .collect();
            let _ = writeln!(out, "Attachments: {}", list.join(", "));
        }
        out.push('\n');
        out.push_str(&truncate(&self.body, STATE_BODY_CHARS));
        DecisionState::Text(out)
    }

    /// The text a k-NN embedder reads: sender, subject, header names,
    /// attachment names and the new (unquoted) text.
    pub fn embedding_text(&self) -> String {
        let headers: Vec<&str> = self.headers.keys().map(String::as_str).collect();
        let attachments: Vec<&str> = self.attachments.iter().map(|a| a.name.as_str()).collect();
        format!(
            "{}\n{}\n{}\n{}\n{}",
            party(&self.from),
            self.subject,
            headers.join(" "),
            attachments.join(" "),
            truncate(&new_text(&self.body), STATE_BODY_CHARS)
        )
    }
}

fn party(p: &Party) -> String {
    if p.name.is_empty() {
        p.address.clone()
    } else {
        format!("{} <{}>", p.name, p.address)
    }
}

fn truncate(text: &str, chars: usize) -> String {
    match text.char_indices().nth(chars) {
        Some((end, _)) => format!("{}…", &text[..end]),
        None => text.to_owned(),
    }
}

/// The body without quoted history: stops at the first reply or forward
/// marker and drops `>` lines.
fn new_text(body: &str) -> String {
    let mut out = String::new();
    for line in body.lines() {
        let trimmed = line.trim();
        if QUOTE_MARKER.is_match(trimmed) {
            break;
        }
        if trimmed.starts_with('>') {
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

// --- Text hygiene ----------------------------------------------------------

/// Panics with the pattern's error: built-in patterns are tested to compile.
fn regex(pattern: &str) -> Regex {
    match Regex::new(pattern) {
        Ok(re) => re,
        Err(err) => panic!("built-in pattern does not compile: {err}"),
    }
}

/// Where quoted history starts: a reply or forward marker. Text hygiene for
/// the k-NN embedder, not a decision.
static QUOTE_MARKER: LazyLock<Regex> = LazyLock::new(|| {
    regex(
        r"(?i)^(?:on .{3,80} wrote:|em .{3,80} escreveu:|el .{3,80} escribió:|le .{3,80} a écrit ?:|-{2,}\s*(?:original message|mensagem original|mensaje original|message d'origine|forwarded message|mensagem encaminhada|mensaje reenviado|message transféré)\s*-{2,}|(?:from|de|von)\s*:\s.+@.+)$",
    )
});

// --- Taught-rule matching (campbooks-cloud#125) ------------------------------
//
// A member's own rule names words or a subject's template; these helpers
// normalise text so the member's condition matches lookalikes. They decide
// nothing on their own (ADR 0018): only a rule a member taught uses them.

/// The words of a subject or phrase, lower-cased with accents dropped, in
/// order (letters and digits only).
pub fn subject_words(text: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut word = String::new();
    for c in text.chars().flat_map(char::to_lowercase) {
        if let Some(plain) = fold_accent(c) {
            word.push_str(plain);
        } else if c.is_alphanumeric() {
            word.push(c);
        } else if !word.is_empty() {
            words.push(std::mem::take(&mut word));
        }
    }
    if !word.is_empty() {
        words.push(word);
    }
    words
}

/// The plain letters of an accented Latin letter (pt, es, fr).
fn fold_accent(c: char) -> Option<&'static str> {
    Some(match c {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' => "a",
        'ç' => "c",
        'è' | 'é' | 'ê' | 'ë' => "e",
        'ì' | 'í' | 'î' | 'ï' => "i",
        'ñ' => "n",
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' => "o",
        'ù' | 'ú' | 'û' | 'ü' => "u",
        'ý' | 'ÿ' => "y",
        'œ' => "oe",
        'æ' => "ae",
        'ß' => "ss",
        _ => return None,
    })
}

static REPLY_PREFIX: LazyLock<Regex> = LazyLock::new(|| {
    regex(r"(?i)^\s*(?:re|fwd?|fw|enc|res|rv|tr|réf|ref|aw|wg)\s*(?:\[\d+\])?\s*:\s*")
});
static DATE_TOKEN: LazyLock<Regex> =
    LazyLock::new(|| regex(r"\b\d{1,4}[-/.]\d{1,2}[-/.]\d{1,4}\b"));
static ID_TOKEN: LazyLock<Regex> = LazyLock::new(|| regex(r"[\p{L}\d_-]{6,}"));
static DIGITS: LazyLock<Regex> = LazyLock::new(|| regex(r"\d+"));
static HASHES: LazyLock<Regex> = LazyLock::new(|| regex(r"#(?:[\s#.,:/-]*#)*"));
static SPACES: LazyLock<Regex> = LazyLock::new(|| regex(r"\s+"));
static LITERAL_WORD: LazyLock<Regex> = LazyLock::new(|| regex(r"\p{L}{2,}"));

/// Longest subject template, in characters.
pub const TEMPLATE_MAX_CHARS: usize = 120;

/// A subject with what changes from one message to the next taken out:
/// reply and forward prefixes (in the four locales) dropped, dates and digit
/// runs as `#`, ids (six or more characters mixing letters and digits) as
/// `*`, lower-cased, spaces collapsed, at most [`TEMPLATE_MAX_CHARS`]. `None`
/// when fewer than two literal words are left: such a template would match
/// unrelated mail.
pub fn subject_template(subject: &str) -> Option<String> {
    let mut rest = subject.trim();
    while let Some(m) = REPLY_PREFIX.find(rest) {
        rest = &rest[m.end()..];
    }
    let lower = rest.to_lowercase();
    let no_dates = DATE_TOKEN.replace_all(&lower, "#");
    let no_ids = ID_TOKEN.replace_all(&no_dates, |c: &regex::Captures<'_>| {
        let token = &c[0];
        let letters = token.chars().any(char::is_alphabetic);
        let digits = token.chars().any(|ch| ch.is_ascii_digit());
        if letters && digits {
            "*".to_owned()
        } else {
            token.to_owned()
        }
    });
    let no_digits = DIGITS.replace_all(&no_ids, "#");
    let hashes = HASHES.replace_all(&no_digits, "#");
    let template = SPACES.replace_all(hashes.trim(), " ");
    if LITERAL_WORD.find_iter(&template).count() < 2 {
        return None;
    }
    Some(
        template
            .chars()
            .take(TEMPLATE_MAX_CHARS)
            .collect::<String>()
            .trim_end()
            .to_owned(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mail(from: &str, subject: &str, body: &str) -> Envelope {
        Envelope {
            from: Party {
                name: String::new(),
                address: from.into(),
            },
            subject: subject.into(),
            body: body.into(),
            ..Envelope::default()
        }
    }

    fn with_header(mut env: Envelope, k: &str, v: &str) -> Envelope {
        env.headers.insert(k.into(), v.into());
        env
    }

    #[test]
    fn decision_state_is_compact_and_debug_hides_content() {
        let env = with_header(
            mail("ana@example.com", "Fatura", &"x".repeat(5000)),
            "List-Id",
            "<news.example>",
        );
        let DecisionState::Text(state) = env.decision_state() else {
            unreachable!()
        };
        assert!(state.contains("Subject: Fatura"));
        assert!(state.contains("list-id: <news.example>"));
        // The body is cut at 1,500 characters; the triage context adds
        // about 550.
        assert!(state.chars().count() < 2300);
        assert!(!format!("{env:?}").contains("Fatura"));
    }

    #[test]
    fn quoted_history_is_not_embedded() {
        let env = mail(
            "ana@example.com",
            "Re: lunch",
            "Friday works.\n\nOn Mon, 12 Oct 2026, Sam wrote:\n> Lunch on Friday?",
        );
        let text = env.embedding_text();
        assert!(text.contains("Friday works."), "{text}");
        assert!(!text.contains("Lunch on Friday?"), "{text}");
    }
}
