//! The answers check: does the `decide` model get obvious emails right?
//!
//! A model server can answer every request and still be useless: in
//! October 2026 the multilingual Laya checkpoint answered "yes" to almost
//! every triage question, at confidence 1.000 (campbooks-cloud#128). The
//! reachability probe ([`Gateway::health`]) cannot see that, so the gateway
//! also asks the triage questions about eight fictional emails whose answers
//! are not in doubt (a newsletter needs no reply, a friend asking for a call
//! does, a look-alike domain asking for a password is phishing) and grades
//! the answers.
//!
//! The check runs when the server starts and then periodically
//! ([`Gateway::keep_answers_checked`]): every [`RECHECK_PASSED`] while it
//! passes, every [`RECHECK_FAILED`] while it does not (a model swapped or
//! fixed is picked up within that time, or at once on restart). Nothing runs
//! per request. A model that gets fewer than [`PASS_SHARE`] of the answers
//! right fails: its role is shown as failing the check in the admin System
//! panel, and a [`Model`](super::strategy::Model) strategy built
//! [`checked`](super::strategy::Model::checked) refuses to answer, so the
//! caller falls back to what it does without a model. The log carries the
//! outcome and the counts, never the answers.

use std::{future::Future, sync::Arc, time::Duration};

use chrono::{DateTime, Utc};
use serde::Serialize;
use tokio::sync::watch;

use super::{
    Answer, DecisionContext,
    signals::{
        email::{Attachment, Envelope, Party, Recipient, triage, triage_questions},
        inbox::CONTEXT_VERSION,
    },
};
use crate::{Gateway, Role};

/// Share of the expected answers a model must get right to pass.
pub const PASS_SHARE: f64 = 0.9;
/// How long a passed check stands before it runs again.
pub const RECHECK_PASSED: Duration = Duration::from_hours(6);
/// How soon a failed (or unanswered) check runs again.
pub const RECHECK_FAILED: Duration = Duration::from_mins(15);
/// The purpose the check's calls are traced under.
pub const PURPOSE: &str = "ai.answers_check";
/// Prefix of the `model_version` of answers given while the check passed
/// (`checked:ctx1`, or `checked:ctx1:<provider version>`). Answers without
/// it were never vouched for by a check.
pub const STAMP: &str = "checked:";

/// How the last check ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckOutcome {
    /// At least [`PASS_SHARE`] of the expected answers were right.
    Passed,
    /// Fewer were: the model's answers are not used.
    Failed,
    /// The provider did not answer (down, refused, malformed answers).
    Unavailable,
}

/// The result of one check.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CheckReport {
    /// How it ended.
    pub outcome: CheckOutcome,
    /// Expected answers the model got right.
    pub right: u32,
    /// Expected answers asked.
    pub total: u32,
    /// The checkpoint that answered, when it did.
    pub model: Option<String>,
    /// When the check finished.
    pub checked_at: DateTime<Utc>,
}

impl CheckReport {
    /// Whether the model's answers may be used.
    pub fn passed(&self) -> bool {
        self.outcome == CheckOutcome::Passed
    }
}

/// The answers check of a gateway's `decide` role.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", tag = "state")]
pub enum AnswersCheck {
    /// No check has finished yet.
    Pending,
    /// The last check.
    Done(CheckReport),
}

/// The last check, shared by a gateway's clones.
#[derive(Debug, Clone)]
pub(crate) struct Checks(Arc<watch::Sender<AnswersCheck>>);

impl Default for Checks {
    fn default() -> Self {
        Self(Arc::new(watch::channel(AnswersCheck::Pending).0))
    }
}

/// What a case expects of one question.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Expected {
    /// The statement holds.
    Yes,
    /// It does not.
    No,
    /// This stream.
    Stream(&'static str),
}

/// A fictional email and the answers it must get.
#[derive(Debug, Clone)]
pub struct Case {
    /// A short name, for tests.
    pub name: &'static str,
    /// The email.
    pub envelope: Envelope,
    /// Question id and the answer it must get.
    pub expect: Vec<(&'static str, Expected)>,
}

fn alex() -> Recipient {
    Recipient {
        name: "Alex Morgan".into(),
        addresses: vec!["alex@example.com".into()],
        shared: false,
    }
}

fn party(name: &str, address: &str) -> Party {
    Party {
        name: name.into(),
        address: address.into(),
    }
}

fn email(from: Party, subject: &str, headers: &[(&str, &str)], body: &str) -> Envelope {
    Envelope {
        from,
        to: vec![party("Alex Morgan", "alex@example.com")],
        subject: subject.into(),
        headers: headers
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect(),
        body: body.into(),
        recipient: Some(alex()),
        ..Envelope::default()
    }
}

/// The eight fictional emails and what each must get: 33 expected answers
/// in all. Every sender is on a reserved domain except the code-review
/// notification's, which uses GitHub's real notification address so the
/// case looks like the mail it stands for; its text is made up.
pub fn cases() -> Vec<Case> {
    vec![
        newsletter(),
        friend_asks_for_a_call(),
        code_review_notification(),
        phishing(),
        shop_receipt(),
        review_by_friday(),
        password_notice(),
        colleague_update(),
    ]
}

fn newsletter() -> Case {
    use Expected::{No, Stream};
    use triage::{ASKS_ACTION, NEEDS_REPLY, STREAM, SUSPICIOUS};
    Case {
        name: "newsletter",
        envelope: email(
            party("The Weekly Brief", "news@weeklybrief.example.com"),
            "This week: 5 tools for small teams",
            &[
                (
                    "List-Unsubscribe",
                    "<https://weeklybrief.example.com/unsub>",
                ),
                ("List-Id", "<weekly.weeklybrief.example.com>"),
            ],
            "Hello reader,\n\nThis week we look at five tools that help small teams plan \
             their work. Read the full issue on our site.\n\nYou are receiving this because \
             you subscribed. Unsubscribe at any time.",
        ),
        expect: vec![
            (STREAM, Stream("newsletters")),
            (NEEDS_REPLY, No),
            (ASKS_ACTION, No),
            (SUSPICIOUS, No),
        ],
    }
}

fn friend_asks_for_a_call() -> Case {
    use Expected::{No, Stream, Yes};
    use triage::{ASKS_ACTION, NEEDS_REPLY, STREAM, SUSPICIOUS};
    Case {
        name: "a friend asks for a call",
        envelope: email(
            party("Marta Silva", "marta@example.org"),
            "Quick call?",
            &[],
            "Hi Alex,\n\nCan you call me tomorrow? I want to ask you about the trip in May. \
             Let me know what time works for you.\n\nThanks!\nMarta",
        ),
        expect: vec![
            (STREAM, Stream("primary")),
            (NEEDS_REPLY, Yes),
            (ASKS_ACTION, Yes),
            (SUSPICIOUS, No),
        ],
    }
}

fn code_review_notification() -> Case {
    use Expected::{No, Stream};
    use triage::{HAS_DEADLINE, NEEDS_REPLY, STREAM, SUSPICIOUS};
    let mut notification = email(
        party("Sam Doe", "notifications@github.com"),
        "Re: [example/widgets] Fix date parsing in the importer (PR #128)",
        &[
            ("List-Unsubscribe", "<mailto:unsub@reply.github.com>"),
            ("List-Id", "example/widgets <widgets.example.github.com>"),
        ],
        "@sam-doe commented on this pull request.\n\nLooks good to me, the new test covers the \
         leap year case.\n\nReply to this email directly, view it on GitHub, or \
         unsubscribe.\nYou are receiving this because you were mentioned.",
    );
    notification.to = vec![party("", "widgets@noreply.github.com")];
    Case {
        name: "a code review notification",
        envelope: notification,
        expect: vec![
            (STREAM, Stream("updates")),
            (NEEDS_REPLY, No),
            (HAS_DEADLINE, No),
            (SUSPICIOUS, No),
        ],
    }
}

fn phishing() -> Case {
    use Expected::{No, Yes};
    use triage::{NEEDS_REPLY, SUSPICIOUS};
    Case {
        name: "phishing",
        envelope: email(
            party("PayPaI Security", "security@paypa1-verify.example.net"),
            "Your account has been suspended",
            &[],
            "Dear customer,\n\nWe detected unusual activity. Your account will be closed \
             within 24 hours unless you verify your password and card number at the link \
             below:\n\nhttp://paypa1-verify.example.net/login\n\nPayPaI Security Team",
        ),
        expect: vec![(SUSPICIOUS, Yes), (NEEDS_REPLY, No)],
    }
}

fn shop_receipt() -> Case {
    use Expected::{No, Stream};
    use triage::{ASKS_ACTION, HAS_DEADLINE, NEEDS_REPLY, STREAM, SUSPICIOUS};
    Case {
        name: "a shop receipt",
        envelope: email(
            party("Paper & Co", "orders@paperco.example.com"),
            "Your receipt for order 4417",
            &[],
            "Thank you for your order.\n\nOrder 4417\n2 notebooks  EUR 18.00\nTotal paid: \
             EUR 18.00 (card ending 4242)\n\nYour order will ship in two days.",
        ),
        expect: vec![
            (STREAM, Stream("receipts")),
            (NEEDS_REPLY, No),
            (ASKS_ACTION, No),
            (HAS_DEADLINE, No),
            (SUSPICIOUS, No),
        ],
    }
}

fn review_by_friday() -> Case {
    use Expected::{No, Stream, Yes};
    use triage::{ASKS_ACTION, HAS_DEADLINE, NEEDS_REPLY, STREAM, SUSPICIOUS};
    let mut review = email(
        party("Rita Gomes", "rita@gomes-legal.example"),
        "Contract for the studio lease",
        &[],
        "Hello Alex,\n\nAttached is the draft lease. Could you review it and send me your \
         comments by Friday? The landlord wants an answer before the weekend.\n\nBest \
         regards,\nRita",
    );
    review.attachments = vec![Attachment {
        name: "lease-draft.pdf".into(),
        content_type: "application/pdf".into(),
    }];
    Case {
        name: "a client asks for a review by Friday",
        envelope: review,
        expect: vec![
            (STREAM, Stream("primary")),
            (NEEDS_REPLY, Yes),
            (ASKS_ACTION, Yes),
            (HAS_DEADLINE, Yes),
            (SUSPICIOUS, No),
        ],
    }
}

fn password_notice() -> Case {
    use Expected::{No, Stream};
    use triage::{HAS_DEADLINE, NEEDS_REPLY, STREAM, SUSPICIOUS};
    Case {
        name: "a password change notice",
        envelope: email(
            party("Notes App", "no-reply@notesapp.example.com"),
            "Your password was changed",
            &[("Auto-Submitted", "auto-generated")],
            "Hi Alex,\n\nThe password for your Notes App account was changed on 12 October \
             at 09:14. No further action is needed.",
        ),
        expect: vec![
            (STREAM, Stream("updates")),
            (NEEDS_REPLY, No),
            (HAS_DEADLINE, No),
            (SUSPICIOUS, No),
        ],
    }
}

fn colleague_update() -> Case {
    use Expected::{No, Stream};
    use triage::{ASKS_ACTION, HAS_DEADLINE, NEEDS_REPLY, STREAM, SUSPICIOUS};
    Case {
        name: "a colleague shares an update",
        envelope: email(
            party("Rui Matos", "rui@example.com"),
            "Site visit went fine",
            &[],
            "Alex,\n\nJust back from the site. The scaffolding is up and the plumber \
             finished the first fix. Nothing for you to do, I will send photos next \
             week.\n\nRui",
        ),
        expect: vec![
            (STREAM, Stream("primary")),
            (NEEDS_REPLY, No),
            (ASKS_ACTION, No),
            (HAS_DEADLINE, No),
            (SUSPICIOUS, No),
        ],
    }
}

/// How many of `case`'s expectations `answers` meet: `(right, asked)`.
pub fn grade(case: &Case, answers: &[Answer]) -> (u32, u32) {
    let mut right = 0;
    let mut asked = 0;
    for (question, expected) in &case.expect {
        asked += 1;
        let Some(answer) = answers.iter().find(|a| a.question_id == *question) else {
            continue;
        };
        let ok = match expected {
            Expected::Yes => answer.noul() == Some(true),
            Expected::No => answer.noul() == Some(false),
            Expected::Stream(stream) => answer.choice() == Some(stream),
        };
        right += u32::from(ok);
    }
    (right, asked)
}

/// The outcome of `right` out of `total`.
pub fn outcome(right: u32, total: u32) -> CheckOutcome {
    if total > 0 && f64::from(right) >= PASS_SHARE * f64::from(total) {
        CheckOutcome::Passed
    } else {
        CheckOutcome::Failed
    }
}

/// The `model_version` an answer given under a passed check is stored with.
pub fn stamp(provider_version: Option<&str>) -> String {
    match provider_version {
        Some(version) => format!("{STAMP}ctx{CONTEXT_VERSION}:{version}"),
        None => format!("{STAMP}ctx{CONTEXT_VERSION}"),
    }
}

impl Gateway {
    /// The last answers check of the `decide` role.
    pub fn answers_check(&self) -> AnswersCheck {
        self.checks.0.borrow().clone()
    }

    /// Record `report` as the last check without running one: for tests of
    /// the modules that gate on it, and for tools that grade a model
    /// elsewhere.
    pub fn record_answers_check(&self, report: CheckReport) {
        self.checks.0.send_replace(AnswersCheck::Done(report));
    }

    /// Ask the [`cases`] of the `decide` role now, publish the report for
    /// [`Gateway::answers_check`] and log its outcome and counts.
    pub async fn run_answers_check(&self) -> CheckReport {
        let questions = triage_questions();
        let ctx = DecisionContext::new(PURPOSE);
        let mut right = 0;
        let mut total = 0;
        let mut model = None;
        let mut failed = false;
        for case in cases() {
            // The instance's own check: counted against no workspace.
            let state = case.envelope.decision_state();
            let decided = self.decide(&ctx, &state, &questions);
            match crate::usage::Scope::instance(PURPOSE).run(decided).await {
                Ok(answers) => {
                    let answers: Vec<Answer> = answers.iter().cloned().collect();
                    let (r, t) = grade(&case, &answers);
                    right += r;
                    total += t;
                    if model.is_none() {
                        model = answers.first().map(|a| a.model.clone());
                    }
                }
                Err(error) => {
                    tracing::warn!(%error, "decide answers check: the provider did not answer");
                    failed = true;
                    break;
                }
            }
        }
        let report = CheckReport {
            outcome: if failed {
                CheckOutcome::Unavailable
            } else {
                outcome(right, total)
            },
            right,
            total,
            model,
            checked_at: Utc::now(),
        };
        match report.outcome {
            CheckOutcome::Passed => tracing::info!(
                right = report.right,
                total = report.total,
                "decide answers check passed"
            ),
            CheckOutcome::Failed => tracing::warn!(
                right = report.right,
                total = report.total,
                "decide answers check failed: the decide model's answers are not used"
            ),
            CheckOutcome::Unavailable => {
                tracing::warn!("decide answers check could not run: answers are not used");
            }
        }
        self.record_answers_check(report.clone());
        report
    }

    /// Run the answers check now and then again every [`RECHECK_PASSED`]
    /// (passed) or [`RECHECK_FAILED`] (otherwise) until `stop` resolves.
    /// Returns at once when the `decide` role is not configured or may not
    /// receive mail (then nothing would use its answers).
    pub async fn keep_answers_checked(&self, stop: impl Future<Output = ()>) {
        if self.config().provider_for_content(Role::Decide).is_err() {
            return;
        }
        tokio::pin!(stop);
        loop {
            let report = self.run_answers_check().await;
            let wait = if report.passed() {
                RECHECK_PASSED
            } else {
                RECHECK_FAILED
            };
            tokio::select! {
                () = &mut stop => return,
                () = tokio::time::sleep(wait) => {}
            }
        }
    }

    /// Whether the `decide` role's answers may be used: its last check
    /// passed. Never waits: before the first check finishes the answer is
    /// no, so callers carry on without the model at once.
    pub fn answers_trusted(&self) -> bool {
        matches!(&*self.checks.0.borrow(), AnswersCheck::Done(report) if report.passed())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn cases_cover_both_answers_on_reserved_domains() {
        let cases = cases();
        assert_eq!(cases.len(), 8);
        let total: usize = cases.iter().map(|c| c.expect.len()).sum();
        assert_eq!(total, 33);
        for question in [
            triage::NEEDS_REPLY,
            triage::ASKS_ACTION,
            triage::HAS_DEADLINE,
            triage::SUSPICIOUS,
        ] {
            let answers: HashSet<_> = cases
                .iter()
                .flat_map(|c| &c.expect)
                .filter(|(q, _)| *q == question)
                .map(|(_, e)| *e)
                .collect();
            assert!(
                answers.contains(&Expected::Yes) && answers.contains(&Expected::No),
                "{question} needs a yes and a no case, or always-yes would pass"
            );
        }
        for case in &cases {
            let domain = case.envelope.from.address.rsplit('@').next().unwrap();
            assert!(
                domain.ends_with(".example")
                    || domain.ends_with("example.com")
                    || domain.ends_with("example.org")
                    || domain.ends_with("example.net")
                    || domain == "github.com",
                "{}: {domain}",
                case.name
            );
        }
    }

    #[test]
    fn ninety_percent_passes() {
        assert_eq!(outcome(30, 33), CheckOutcome::Passed);
        assert_eq!(outcome(29, 33), CheckOutcome::Failed);
        assert_eq!(outcome(0, 0), CheckOutcome::Failed);
    }

    #[test]
    fn stamps_carry_the_context_version() {
        assert_eq!(stamp(None), "checked:ctx1");
        assert_eq!(stamp(Some("fp_1")), "checked:ctx1:fp_1");
    }
}
