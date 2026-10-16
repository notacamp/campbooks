//! The facts strategy: what deterministic code may know about an email
//! (ADR 0018).
//!
//! Understanding what an email says is a model's job. Deterministic code
//! reads structured facts only, and from them may only say "no":
//!
//! - [`Facts::of`] copies, from an [`Envelope`], the sender's and
//!   recipients' addresses, the headers `List-Id`, `List-Unsubscribe`,
//!   `Precedence`, `Auto-Submitted`, `Authentication-Results` and
//!   `Reply-To`, and the attachments' media types. Nothing else: a rule
//!   written against [`Facts`] cannot read what the email says.
//! - [`EmailFacts`] answers `needs_reply` false, and only that, for list
//!   mail, mail sent automatically and mail from a no-reply address. It
//!   abstains on every other question, and never answers true.
//! - [`Facts::hints`] is what a `decide` model is told about the same facts
//!   ([`super::inbox::preamble`]): hints, not decisions.
//!
//! A test reads this file and fails if it names a message's text, so a
//! content rule cannot come back through this module.

use std::{sync::LazyLock, time::Instant};

use regex::Regex;

use super::{
    auth_results,
    email::{Envelope, triage},
};
use crate::{
    Error,
    decide::{
        Question, QuestionKind,
        strategy::{BoxFuture, Decided, Decisions, Reason, Strategy, local_answer, peaked},
    },
};

/// How sure the facts are of their "no".
pub const CONFIDENCE: f64 = 0.99;

/// Rule id: a mailing-list header (`List-Id`, `List-Unsubscribe`, or a
/// `Precedence` of `bulk`, `list` or `junk`).
pub const LIST_MAIL: &str = "facts.list_mail";
/// Rule id: an `Auto-Submitted` header other than `no`.
pub const AUTOMATIC: &str = "facts.automatic";
/// Rule id: a no-reply sender address.
pub const NO_REPLY_SENDER: &str = "facts.no_reply_sender";

/// Local parts that say no one reads replies, each optionally followed by
/// `+`, `.`, `-` or `_` and more (`no-reply+tag`, `bounces-42`).
const NO_REPLY_LOCAL_PARTS: [&str; 8] = [
    "noreply",
    "no-reply",
    "donotreply",
    "do-not-reply",
    "mailer-daemon",
    "postmaster",
    "bounce",
    "bounces",
];

/// Local parts a service sends from (`billing@`, `notifications@`, `team@`),
/// in the four languages Campbooks ships in. A hint for the model, never a
/// decision.
static SERVICE_LOCAL_PART: LazyLock<Regex> = LazyLock::new(|| {
    match Regex::new(
        r"(?i)^(?:no-?reply|do-?not-?reply|nao-?responder|naoresponder|no-?responder|ne-?pas-?repondre|notifica(?:tions?|coes|ciones)|alerts?|alertas?|info|news(?:letter)?|novidades|noticias|actualites|billing|invoices?|faturas?|facturas?|factur(?:acion|ation)|faturacao|mailer|mailer-daemon|accounts?|conta|cuenta|compte|security|seguranca|seguridad|securite|updates?|marketing|promo(?:tions?|s)?|hello|ola|hola|bonjour|team|equipa|equipo|equipe|orders?|encomendas|pedidos|commandes|receipts?|recibos|recus|payments?|pagamentos|pagos|paiements|support|suporte|soporte|service|servicos|servicios|clientes?|customer\w*|events?|eventos|evenements|digest|reminders?|lembretes|recordatorios|rappels|calendar|calendario|agenda|shipping|envios|expedition|livraison|entregas|citas|rdv|rendez-?vous|marcacoes|avisos|cobran(?:zas|cas)|cobros|contas|cuentas|comptes|alertes|factures|reservas|reservations|bookings?|tickets|bilheteira|entradas|monitor(?:ing|izacao)?|status|system|sistema|systeme)(?:[._+-].*)?$",
    ) {
        Ok(re) => re,
        Err(err) => panic!("the service address pattern does not compile: {err}"),
    }
});

/// Documentation domains (RFC 2606) the registrable-domain guess treats as
/// suffixes, so fixtures can stand for many organisations.
const EXAMPLE_SUFFIXES: [&str; 3] = ["example.com", "example.org", "example.net"];

/// Whether `domain` is itself a public suffix of the Public Suffix List
/// (`com`, `co.uk`, and the private ones such as `github.io`), under which
/// anyone registers a domain: a taught `sender_domain` may not name one
/// alone. The ICANN and the private sections both count (see the
/// `harden-security-alert-auth` design). A name under a TLD the list does
/// not know is read as having that TLD as its suffix, as the list's default
/// rule says.
pub fn is_public_suffix(domain: &str) -> bool {
    let domain = domain.trim().trim_matches('.').to_ascii_lowercase();
    !domain.is_empty() && psl::domain_str(&domain).is_none()
}

/// A best-effort registrable domain of an address or a host
/// (`notifications@mail.github.com` → `github.com`).
pub fn registrable_domain(address_or_host: &str) -> String {
    let host = address_or_host
        .rsplit_once('@')
        .map_or(address_or_host, |(_, d)| d);
    registrable(host)
}

/// Whether a machine sent it, from the facts alone: list headers,
/// `Auto-Submitted`, or a service-like sender address (`no-reply@`,
/// `notifications@`, `billing@`, …).
pub fn is_machine_sender(envelope: &Envelope) -> bool {
    let facts = Facts::of(envelope);
    facts.list_header().is_some()
        || facts.automatic().is_some()
        || facts.no_reply_sender()
        || facts.service_sender()
}

/// The structured facts of an email: addresses, a few headers and the
/// attachments' media types. It holds none of the email's text.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Facts {
    /// The sender's address, lower-cased.
    pub from: String,
    /// The recipients' and copied recipients' addresses, lower-cased.
    pub recipients: Vec<String>,
    /// `List-Id`.
    pub list_id: Option<String>,
    /// `List-Unsubscribe`.
    pub list_unsubscribe: Option<String>,
    /// `Precedence`.
    pub precedence: Option<String>,
    /// `Auto-Submitted`.
    pub auto_submitted: Option<String>,
    /// `Authentication-Results` (the receiving provider's).
    pub authentication_results: Option<String>,
    /// `Reply-To`.
    pub reply_to: Option<String>,
    /// The attachments' media types.
    pub attachment_types: Vec<String>,
}

impl Facts {
    /// The facts of `envelope`. The only code that reads an [`Envelope`]
    /// here, and it reads no text.
    pub fn of(envelope: &Envelope) -> Self {
        let header = |name: &str| envelope.header(name).map(str::to_owned);
        Self {
            from: envelope.from.address.to_lowercase(),
            recipients: envelope
                .to
                .iter()
                .chain(&envelope.cc)
                .map(|party| party.address.to_lowercase())
                .collect(),
            list_id: header("list-id"),
            list_unsubscribe: header("list-unsubscribe"),
            precedence: header("precedence"),
            auto_submitted: header("auto-submitted"),
            authentication_results: header("authentication-results"),
            reply_to: header("reply-to"),
            attachment_types: envelope
                .attachments
                .iter()
                .map(|attachment| attachment.content_type.to_lowercase())
                .collect(),
        }
    }

    /// The mailing-list header that marks list mail, if any.
    pub fn list_header(&self) -> Option<&'static str> {
        if self.list_id.is_some() {
            Some("List-Id")
        } else if self.list_unsubscribe.is_some() {
            Some("List-Unsubscribe")
        } else {
            self.precedence
                .as_deref()
                .map(str::to_ascii_lowercase)
                .filter(|p| ["bulk", "list", "junk"].iter().any(|v| p.contains(v)))
                .map(|_| "Precedence")
        }
    }

    /// The `Auto-Submitted` value, when it says a machine sent the mail.
    pub fn automatic(&self) -> Option<String> {
        self.auto_submitted
            .as_deref()
            .map(|v| v.trim().to_ascii_lowercase())
            .filter(|v| !v.is_empty() && v != "no")
    }

    /// Whether the sender's address is one no one reads replies to.
    pub fn no_reply_sender(&self) -> bool {
        let local = local_part(&self.from);
        NO_REPLY_LOCAL_PARTS.iter().any(|base| {
            local == *base
                || local
                    .strip_prefix(base)
                    .is_some_and(|rest| rest.starts_with(['+', '.', '-', '_']))
        })
    }

    /// Whether the sender's address looks like a service's (`billing@`,
    /// `notifications@`).
    pub fn service_sender(&self) -> bool {
        SERVICE_LOCAL_PART.is_match(local_part(&self.from))
    }

    /// Whether a `Reply-To` address is on another domain than the sender's.
    pub fn reply_elsewhere(&self) -> bool {
        let from = registrable(domain(&self.from));
        self.reply_to.as_deref().is_some_and(|reply_to| {
            reply_to
                .split(',')
                .map(str::trim)
                .filter(|address| address.contains('@'))
                .any(|address| registrable(domain(address)) != from)
        })
    }

    /// Whether the receiving provider's `Authentication-Results` say the
    /// sender's domain failed: DMARC failed, or neither a DMARC pass for
    /// the From domain nor a DKIM pass aligned with it. Absent results say
    /// nothing either way.
    pub fn authentication_failed(&self) -> bool {
        let Some(value) = self.authentication_results.as_deref() else {
            return false;
        };
        // A header too long to be a provider's (padded by a sender to hide a
        // failure) is a failure: only an absent or unreadable one says nothing.
        if auth_results::is_oversized(value) {
            return true;
        }
        let Some(results) = auth_results::parse(value) else {
            return false;
        };
        let dmarc_failed = results
            .iter()
            .any(|r| r.method == "dmarc" && r.result == "fail");
        dmarc_failed || !self.authenticated(&results)
    }

    /// Whether `results` authenticate the From domain: exactly one `dmarc`
    /// result, `pass`, whose `header.from` is the From domain; or a
    /// `dkim=pass` whose signing domains all have the From domain's
    /// registrable domain, provided no `dmarc` result refuses it (`fail`,
    /// `quarantine`, `reject`, `policy`: the domain's policy said no). `none`
    /// (no policy published, RFC 7489 §11.2), `bestguesspass`, `temperror`
    /// and `permerror` do not refuse: many small senders sign with DKIM and
    /// publish no DMARC.
    fn authenticated(&self, results: &[auth_results::MethodResult]) -> bool {
        let host = domain(&self.from).to_ascii_lowercase();
        let from = registrable(&host);
        if !plain_domain(&host) || from.is_empty() || is_public_suffix(&host) {
            return false;
        }
        let dmarc: Vec<_> = results.iter().filter(|r| r.method == "dmarc").collect();
        let dmarc_pass = match dmarc.as_slice() {
            [only] => {
                let headers_from: Vec<_> = only.values("header.from").collect();
                only.result == "pass"
                    && matches!(headers_from.as_slice(), [d] if d.eq_ignore_ascii_case(&host))
            }
            _ => false,
        };
        let aligned = |signer: &str| {
            let signer = signer.rsplit('@').next().unwrap_or_default();
            plain_domain(signer) && registrable(signer) == from
        };
        let dmarc_refused = dmarc.iter().any(|r| {
            matches!(
                r.result.as_str(),
                "fail" | "quarantine" | "reject" | "policy"
            )
        });
        let dkim_pass = !dmarc_refused
            && results.iter().any(|r| {
                let signers: Vec<_> = r.values("header.d").chain(r.values("header.i")).collect();
                r.method == "dkim"
                    && r.result == "pass"
                    && !signers.is_empty()
                    && signers.into_iter().all(aligned)
            });
        dmarc_pass || dkim_pass
    }

    /// Why the facts say no reply is needed, if they do: the first of list
    /// mail, automatic mail and a no-reply sender.
    pub fn no_reply_needed(&self) -> Option<Reason> {
        if let Some(header) = self.list_header() {
            return Some(Reason::new(LIST_MAIL, header));
        }
        if let Some(value) = self.automatic() {
            return Some(Reason::new(AUTOMATIC, format!("Auto-Submitted: {value}")));
        }
        if self.no_reply_sender() {
            return Some(Reason::new(NO_REPLY_SENDER, local_part(&self.from)));
        }
        None
    }

    /// What a `decide` model is told about these facts.
    pub fn hints(&self) -> Hints {
        Hints {
            list: self.list_header().is_some(),
            list_id: self.list_id.clone(),
            automatic: self.automatic().is_some(),
            no_reply_sender: self.no_reply_sender(),
            service_sender: self.service_sender(),
            reply_elsewhere: self.reply_elsewhere(),
            authentication_failed: self.authentication_failed(),
        }
    }
}

/// What the facts say about an email, for a model's context.
// Independent yes/no facts, each read on its own: not a state machine.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Hints {
    /// List headers (`List-Id`, `List-Unsubscribe`, a bulk `Precedence`).
    pub list: bool,
    /// The `List-Id` value, when given (sender-controlled: quoted).
    pub list_id: Option<String>,
    /// An `Auto-Submitted` header other than `no`.
    pub automatic: bool,
    /// A no-reply sender address.
    pub no_reply_sender: bool,
    /// A sender address shaped like a service's (`billing@`, `team@`).
    pub service_sender: bool,
    /// A `Reply-To` on another domain than the sender's.
    pub reply_elsewhere: bool,
    /// The receiving provider's authentication of the sender failed.
    pub authentication_failed: bool,
}

/// The facts as a [`Strategy`]: `needs_reply` false for list mail,
/// automatic mail and no-reply senders; an abstention on every other
/// question and whenever no fact applies. It never answers true.
#[derive(Debug, Clone, Copy, Default)]
pub struct EmailFacts;

impl EmailFacts {
    /// Answer synchronously; same result as [`Strategy::decide`].
    pub fn answer(envelope: &Envelope, questions: &[Question]) -> Decisions {
        let started = Instant::now();
        let facts = Facts::of(envelope);
        let answers = questions
            .iter()
            .map(|q| match (q.id.as_str(), &q.kind) {
                (triage::NEEDS_REPLY, QuestionKind::Noul { .. }) => {
                    facts.no_reply_needed().map(|reason| Decided {
                        answer: local_answer(q, peaked(2, 0, CONFIDENCE), "facts", started),
                        reasons: vec![reason],
                    })
                }
                _ => None,
            })
            .collect();
        Decisions {
            answers,
            remote_calls: 0,
        }
    }
}

impl Strategy for EmailFacts {
    fn name(&self) -> &'static str {
        "facts"
    }

    fn decide<'a>(
        &'a self,
        envelope: &'a Envelope,
        questions: &'a [Question],
    ) -> BoxFuture<'a, Result<Decisions, Error>> {
        Box::pin(async move { Ok(Self::answer(envelope, questions)) })
    }
}

/// The registrable domain of `host` by the Public Suffix List (ICANN and
/// private sections): `conta.bancoatlantico.example` →
/// `bancoatlantico.example`, `a.b.co.uk` → `b.co.uk`, `x.github.io` →
/// `x.github.io`. A host the list cannot split (a bare suffix, an unknown
/// TLD alone, an empty label) is returned as it is, never shortened to a
/// name another organisation could share.
fn registrable(host: &str) -> String {
    let host = host.trim_end_matches('.').to_lowercase();
    let host = host.split(':').next().unwrap_or_default();
    if host.split('.').any(str::is_empty) {
        return host.to_owned();
    }
    if EXAMPLE_SUFFIXES
        .iter()
        .any(|s| host == *s || host.ends_with(&format!(".{s}")))
    {
        let labels: Vec<&str> = host.split('.').collect();
        return labels[labels.len().saturating_sub(3)..].join(".");
    }
    psl::domain_str(host).unwrap_or(host).to_owned()
}

/// Whether `host` is a plain ASCII domain: no internationalised label
/// (`xn--`, or Unicode a parser left as is), which can look like another.
fn plain_domain(host: &str) -> bool {
    !host.is_empty()
        && host.is_ascii()
        && host
            .split('.')
            .all(|label| !label.to_ascii_lowercase().starts_with("xn--"))
}

fn domain(address: &str) -> &str {
    address
        .trim()
        .trim_end_matches('>')
        .rsplit_once('@')
        .map_or("", |(_, d)| d)
}

fn local_part(address: &str) -> &str {
    address.split_once('@').map_or(address, |(l, _)| l)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::decide::{
        AnswerValue,
        signals::email::{Attachment, Party, triage_questions},
    };

    fn mail(from: &str, subject: &str, body: &str, headers: &[(&str, &str)]) -> Envelope {
        Envelope {
            from: Party {
                name: "Sender".into(),
                address: from.into(),
            },
            to: vec![Party {
                name: "Sam".into(),
                address: "sam@example.com".into(),
            }],
            subject: subject.into(),
            headers: headers
                .iter()
                .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
                .collect::<BTreeMap<_, _>>(),
            body: body.into(),
            ..Envelope::default()
        }
    }

    fn answers(env: &Envelope) -> Vec<(String, Option<Decided>)> {
        let questions = triage_questions();
        let decided = EmailFacts::answer(env, &questions).answers;
        questions.into_iter().map(|q| q.id).zip(decided).collect()
    }

    /// The `needs_reply` answer, and that every other question abstained.
    fn needs_reply(env: &Envelope) -> Option<Decided> {
        let mut found = None;
        for (id, decided) in answers(env) {
            if id == triage::NEEDS_REPLY {
                found = decided;
            } else {
                assert!(decided.is_none(), "{id} must abstain");
            }
        }
        found
    }

    #[test]
    fn a_newsletter_with_a_question_needs_no_reply() {
        let env = mail(
            "news@paperkite.example",
            "Issue 142",
            "What did you think? Hit reply!",
            &[("List-Unsubscribe", "<mailto:u@paperkite.example>")],
        );
        let decided = needs_reply(&env).expect("the facts answer");
        assert_eq!(decided.answer.value, AnswerValue::Noul(false));
        assert_eq!(decided.answer.confidence, Some(CONFIDENCE));
        assert_eq!(decided.answer.model, "facts");
        assert_eq!(
            decided.reasons,
            [Reason::new(LIST_MAIL, "List-Unsubscribe")]
        );
    }

    #[test]
    fn a_shipping_notice_needs_no_reply_whatever_it_asks() {
        let env = mail(
            "orders@shop.example",
            "Your order has shipped",
            "Questions? Contact our support team.",
            &[("List-Id", "<orders.shop.example>")],
        );
        assert_eq!(
            needs_reply(&env).unwrap().answer.value,
            AnswerValue::Noul(false)
        );
    }

    #[test]
    fn a_persons_question_is_left_to_the_model() {
        let env = mail(
            "rui@costaobras.example",
            "Tiles",
            "Do you want me to order the tiles?",
            &[],
        );
        assert!(answers(&env).iter().all(|(_, decided)| decided.is_none()));
    }

    #[test]
    fn automatic_mail_needs_no_reply() {
        let env = mail(
            "ana@example.org",
            "Out of office",
            "Back Monday.",
            &[("Auto-Submitted", "auto-replied")],
        );
        assert_eq!(
            needs_reply(&env).unwrap().reasons,
            [Reason::new(AUTOMATIC, "Auto-Submitted: auto-replied")]
        );
        let human = mail("ana@example.org", "Hi", "Hi", &[("Auto-Submitted", "no")]);
        assert!(needs_reply(&human).is_none());
    }

    #[test]
    fn no_reply_senders_need_no_reply() {
        for address in [
            "no-reply@clouddrive.example",
            "noreply@clouddrive.example",
            "No-Reply+42@clouddrive.example",
            "donotreply@example.com",
            "do-not-reply_eu@example.com",
            "mailer-daemon@example.com",
            "postmaster@example.com",
            "bounces-123@example.com",
            "bounce.x@example.com",
        ] {
            let decided = needs_reply(&mail(address, "x", "x", &[])).expect(address);
            assert_eq!(decided.reasons[0].rule, NO_REPLY_SENDER, "{address}");
        }
        for address in [
            "norah@example.com",
            "bouncer@example.com",
            "billing@acme.example",
            "postmasters-guild@example.com",
        ] {
            assert!(
                needs_reply(&mail(address, "x", "x", &[])).is_none(),
                "{address}"
            );
        }
    }

    #[test]
    fn a_bulk_precedence_is_list_mail_and_a_first_class_one_is_not() {
        let bulk = mail("a@example.com", "x", "x", &[("Precedence", "bulk")]);
        assert_eq!(needs_reply(&bulk).unwrap().reasons[0].rule, LIST_MAIL);
        let other = mail("a@example.com", "x", "x", &[("Precedence", "first-class")]);
        assert!(needs_reply(&other).is_none());
    }

    #[test]
    fn hints_read_addresses_and_headers() {
        let mut env = mail(
            "billing@acme-hosting.example",
            "x",
            "x",
            &[
                ("Reply-To", "collect@other.example"),
                (
                    "Authentication-Results",
                    "mx.example.com; dkim=pass header.d=acme-hosting.example; \
                     dmarc=fail header.from=acme-hosting.example",
                ),
            ],
        );
        env.attachments.push(Attachment {
            name: "invoice.pdf".into(),
            content_type: "application/PDF".into(),
        });
        let facts = Facts::of(&env);
        assert_eq!(facts.attachment_types, ["application/pdf"]);
        assert_eq!(
            facts.hints(),
            Hints {
                service_sender: true,
                reply_elsewhere: true,
                authentication_failed: true,
                ..Hints::default()
            }
        );

        let signed = mail(
            "ana@studio.example",
            "x",
            "x",
            &[
                ("Reply-To", "ana@mail.studio.example"),
                (
                    "Authentication-Results",
                    "mx.example.com; dkim=pass header.d=studio.example",
                ),
            ],
        );
        assert_eq!(Facts::of(&signed).hints(), Hints::default());
        // No results say nothing.
        assert!(!Facts::of(&mail("a@x.example", "x", "x", &[])).authentication_failed());
    }

    #[test]
    fn registrable_domains() {
        assert_eq!(
            registrable("conta.bancoatlantico.example"),
            "bancoatlantico.example"
        );
        assert_eq!(
            registrable("news.atelier.example.com"),
            "atelier.example.com"
        );
        assert_eq!(registrable("example.com"), "example.com");
    }

    #[test]
    fn registrable_domains_follow_the_public_suffix_list() {
        assert_eq!(registrable("mail.github.com"), "github.com");
        assert_eq!(registrable("a.b.co.uk"), "b.co.uk");
        assert_eq!(registrable("x.com.br"), "x.com.br");
        assert_eq!(registrable("a.github.io"), "a.github.io");
        assert_eq!(registrable("a.b.blogspot.com"), "b.blogspot.com");
        // Suffixes the old hand-written list lacked.
        assert_eq!(registrable("shop.example.org.uk"), "example.org.uk");
        assert_eq!(registrable("mail.firma.co.at"), "firma.co.at");
        // Nothing to cut, or a TLD the list does not know.
        assert_eq!(registrable("co.uk"), "co.uk");
        assert_eq!(registrable("localhost"), "localhost");
        assert_eq!(registrable("a.b.corp.internal"), "corp.internal");
        assert_eq!(registrable("a..com"), "a..com");
        assert!(is_public_suffix("co.uk"));
        assert!(is_public_suffix("github.io"));
        assert!(is_public_suffix("com"));
        assert!(!is_public_suffix("github.com"));
        assert!(!is_public_suffix("x.co.uk"));
    }

    /// `authentication_failed` for mail from `from` carrying `results`.
    fn failed(from: &str, results: &str) -> bool {
        Facts::of(&mail(
            from,
            "x",
            "x",
            &[("Authentication-Results", results)],
        ))
        .authentication_failed()
    }

    #[test]
    fn a_signature_dmarc_refused_authenticates_nothing() {
        let dkim = "dkim=pass header.d=studio.example";
        // DKIM passes, the domain's DMARC policy refuses the mail.
        for dmarc in [
            "dmarc=fail (p=REJECT) header.from=studio.example",
            "dmarc=quarantine header.from=studio.example",
            "dmarc=reject header.from=studio.example",
            "dmarc=policy header.from=studio.example",
        ] {
            let results = format!("mx.example.com; {dkim}; {dmarc}");
            assert!(failed("ana@studio.example", &results), "{dmarc}");
        }
        // No policy published, or no verdict: the signature still counts.
        for dmarc in [
            "dmarc=none header.from=studio.example",
            "dmarc=bestguesspass header.from=studio.example",
            "dmarc=temperror header.from=studio.example",
            "dmarc=permerror header.from=studio.example",
        ] {
            let results = format!("mx.example.com; {dkim}; {dmarc}");
            assert!(!failed("ana@studio.example", &results), "{dmarc}");
        }
        // No DMARC result at all: the signature is enough.
        assert!(!failed(
            "ana@studio.example",
            &format!("mx.example.com; {dkim}; spf=pass")
        ));
        assert!(!failed(
            "ana@studio.example",
            &format!("mx.example.com; {dkim}; dmarc=pass header.from=studio.example")
        ));
        // Two DMARC results, one of them not a pass.
        assert!(failed(
            "ana@studio.example",
            &format!(
                "mx.example.com; {dkim}; dmarc=pass header.from=studio.example; \
                 dmarc=fail header.from=studio.example"
            )
        ));
    }

    #[test]
    fn a_padded_results_header_fails_closed() {
        // A sender pads the header past the cap to hide a DMARC failure.
        let padded = format!(
            "mx.example.com; dmarc=fail header.from=studio.example; spf=none (x{})",
            " ".repeat(auth_results::MAX_LEN)
        );
        assert!(padded.len() > auth_results::MAX_LEN);
        assert!(failed("ana@studio.example", &padded));
        // Absent or unreadable results still say nothing.
        assert!(!Facts::of(&mail("ana@studio.example", "x", "x", &[])).authentication_failed());
        assert!(!failed("ana@studio.example", "mx.example.com; dmarc pass"));
    }

    #[test]
    fn a_punycode_suffix_is_a_suffix() {
        assert!(is_public_suffix("xn--55qx5d.cn"));
        assert_eq!(registrable("a.b.xn--55qx5d.cn"), "b.xn--55qx5d.cn");
    }

    #[test]
    fn mail_signed_under_a_shared_suffix_is_not_aligned() {
        // x.com.br signs for y.com.br; a.github.io for b.github.io.
        for (from, signer) in [
            ("ana@y.com.br", "x.com.br"),
            ("ana@b.github.io", "a.github.io"),
        ] {
            let results = format!("mx.example.com; dkim=pass header.d={signer}");
            assert!(failed(from, &results), "{from} signed by {signer}");
        }
        // The same organisation's subdomain still aligns.
        assert!(!failed(
            "ana@mail.y.com.br",
            "mx.example.com; dkim=pass header.d=y.com.br"
        ));
        // A bare public suffix as the sender authenticates nothing.
        assert!(failed(
            "ana@co.uk",
            "mx.example.com; dkim=pass header.d=co.uk"
        ));
    }

    #[test]
    fn unknown_or_reshaped_questions_are_abstained() {
        let env = mail("no-reply@x.example", "x", "x", &[]);
        let qs = [
            Question::noul("other", "Something else."),
            Question::choice(triage::NEEDS_REPLY, "Which?", [("a", ""), ("b", "")]),
        ];
        assert_eq!(EmailFacts::answer(&env, &qs).answers, vec![None, None]);
    }

    /// No rule here may read what an email says (ADR 0018): outside the
    /// tests, this file never names a subject, a body or an attachment's
    /// name, nor the envelope methods that carry them.
    #[test]
    fn the_facts_read_no_content() {
        let source = include_str!("facts.rs");
        let code = source
            .split("#[cfg(test)]")
            .next()
            .expect("the file has code");
        let forbidden = [
            "subject",
            "body",
            ".name",
            "decision_state",
            "embedding_text",
        ];
        for (number, line) in code.lines().enumerate() {
            let line = line.split("//").next().unwrap_or_default();
            for word in forbidden {
                assert!(
                    !line.contains(word),
                    "line {}: the facts strategy reads `{word}`, which is content \
                     (ADR 0018): leave understanding it to the model",
                    number + 1
                );
            }
        }
    }
}
