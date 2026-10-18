//! Taught rules: deterministic condition → action pairs a member taught
//! Campbooks (campbooks-cloud#125).
//!
//! A [`TaughtRule`] holds conditions that must all hold and actions, at most
//! one per [`Slot`] (labels add up). [`evaluate`] reads a message's
//! [`Facts`] and returns, per slot, what the most specific matching rule says
//! ([`Outcome`]). It calls no model and keeps no state: the same facts and the
//! same rules always give the same outcome, so a member can read a rule as a
//! sentence and trust it.
//!
//! Precedence per slot: the highest [`TaughtRule::specificity`], then the
//! latest `confirmed_at`, then the smallest id. Two guards follow: mail that
//! looks suspicious is never hidden (no archive, no read, no move out of
//! Primary, no "needs nothing", no `surface: never`, no "paid"), and a
//! winning `surface: always` keeps every other rule from hiding the mail,
//! the trash and spam included. A rule that puts mail in the trash or spam
//! is the one exception to the first guard: it is about the suspicious mail
//! itself ("Never see emails like this again", campbooks-cloud#169), so it
//! acts on it.
//!
//! [`TaughtRules`] is also a [`Strategy`] for the stream and needs questions,
//! so the eval harness and a [`Cascade`](super::strategy::Cascade) can put it
//! first.

use std::{cmp::Ordering, collections::BTreeSet, time::Instant};

use serde::{Deserialize, Serialize};

use super::{
    Question, QuestionKind,
    signals::email::{Envelope, is_public_suffix, registrable_domain, subject_words, triage},
    strategy::{BoxFuture, Decided, Decisions, Reason, Strategy, local_answer, peaked},
};
use crate::Error;

/// Most conditions a rule may have.
pub const MAX_CONDITIONS: usize = 4;
/// Most actions a rule may have.
pub const MAX_ACTIONS: usize = 4;

/// The stored headers a condition may test.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HeaderName {
    /// `List-Id`.
    ListId,
    /// `List-Unsubscribe`.
    ListUnsubscribe,
    /// `Precedence`.
    Precedence,
    /// `Auto-Submitted`.
    AutoSubmitted,
}

/// A header test: present, or equal to a value (case-insensitive).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct HeaderTest {
    /// Which header.
    pub name: HeaderName,
    /// The value it must have; absent means "present".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
}

/// One test on a message. All of a rule's conditions must hold.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "field", content = "value", rename_all = "snake_case")]
pub enum Condition {
    /// The sender's address is this one (case-insensitive).
    SenderAddress(String),
    /// The sender's registrable domain is this one, or a subdomain of it.
    SenderDomain(String),
    /// The `List-Id` is this one (angle brackets and case ignored).
    ListId(String),
    /// The subject's template ([`subject_template`]) is this one.
    SubjectTemplate(String),
    /// The subject holds these words, in this order, as whole words (case
    /// and accents ignored).
    SubjectContains(String),
    /// A stored header is present, or has a value.
    Header(HeaderTest),
    /// The conversation has this label (its id).
    Label(String),
    /// The conversation is in this stream.
    Stream(String),
    /// The receiving provider's authentication results say the sender's
    /// domain failed its checks (a structured fact, never the content).
    SenderChecksFail,
}

impl Condition {
    /// How narrowly it picks mail: a template, list or header 3; subject
    /// words or an address 2; a domain, label, stream or failed sender
    /// checks 1.
    pub fn weight(&self) -> u32 {
        match self {
            Self::SubjectTemplate(_) | Self::ListId(_) | Self::Header(_) => 3,
            Self::SubjectContains(_) | Self::SenderAddress(_) => 2,
            Self::SenderDomain(_) | Self::Label(_) | Self::Stream(_) | Self::SenderChecksFail => 1,
        }
    }

    fn holds(&self, facts: &Facts<'_>) -> bool {
        match self {
            Self::SenderAddress(address) => facts.sender_address.eq_ignore_ascii_case(address),
            Self::SenderDomain(domain) => {
                // Fail closed: a domain that is (now) a public suffix, like
                // `github.io`, names every site under it and matches nothing.
                if !is_owned_domain(domain) {
                    return false;
                }
                let domain = domain.to_ascii_lowercase();
                let host = facts
                    .sender_address
                    .rsplit_once('@')
                    .map_or("", |(_, d)| d)
                    .to_ascii_lowercase();
                host == domain || host.ends_with(&format!(".{domain}"))
            }
            Self::ListId(list) => facts
                .list_id
                .is_some_and(|l| normalise_list_id(l) == normalise_list_id(list)),
            Self::SubjectTemplate(template) => facts.subject_template == Some(template.as_str()),
            Self::SubjectContains(words) => contains_words(facts.subject, words),
            Self::Header(test) => {
                let value = match test.name {
                    HeaderName::ListId => facts.list_id,
                    HeaderName::ListUnsubscribe => facts.list_unsubscribe,
                    HeaderName::Precedence => facts.precedence,
                    HeaderName::AutoSubmitted => facts.auto_submitted,
                };
                match (&test.value, value) {
                    (_, None) => false,
                    (None, Some(_)) => true,
                    (Some(want), Some(have)) => have.trim().eq_ignore_ascii_case(want.trim()),
                }
            }
            Self::Label(label) => facts.labels.iter().any(|l| l == label),
            Self::Stream(stream) => facts.stream == stream,
            Self::SenderChecksFail => facts.impersonated && !facts.correspondent,
        }
    }
}

/// `always`: never hide this mail; `never`: it never needs anyone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Surface {
    /// Keep it visible: no rule archives it, marks it read or moves it out
    /// of Primary.
    Always,
    /// Raise no need for it.
    Never,
}

/// What a rule does. At most one per [`Slot`]; labels add up.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum Action {
    /// Put the conversation in this stream.
    MoveToStream {
        /// The stream.
        stream: String,
    },
    /// Take it out of the inbox.
    Archive,
    /// Put this label on it (the label's id).
    AddLabel {
        /// The label.
        label: String,
    },
    /// Mark it read.
    MarkRead,
    /// The answer to "needs a reply".
    NeedsReply {
        /// True or false.
        value: bool,
    },
    /// The answer to "asks for an action".
    AsksAction {
        /// True or false.
        value: bool,
    },
    /// Always or never show it.
    Surface {
        /// Which.
        value: Surface,
    },
    /// Put it in the trash.
    Trash,
    /// Report it as spam.
    Spam,
    /// Its bills are paid: Campbooks never asks "Is it paid?" about it.
    MarkPaid,
    /// Show its remote images without asking (campbooks-cloud#200): read
    /// when the mail is opened ([`shows_images`]), never on arrival, and
    /// only for mail whose sender passes authentication.
    ShowImages,
}

/// Where a rule puts mail it takes out of the way for good.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Bin {
    /// The trash.
    Trash,
    /// Spam (reported).
    Spam,
}

/// The places an action fills; one rule fills each at most once.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Slot {
    /// The stream.
    Stream,
    /// Archived.
    Archive,
    /// Read.
    Read,
    /// A label (additive).
    Label,
    /// "Needs a reply".
    NeedsReply,
    /// "Asks for an action".
    AsksAction,
    /// Always or never show.
    Surface,
    /// The trash or spam.
    Bin,
    /// Whether its bills are paid.
    Money,
    /// Whether its remote images show without asking.
    Images,
}

impl Action {
    /// The slot it fills.
    pub fn slot(&self) -> Slot {
        match self {
            Self::MoveToStream { .. } => Slot::Stream,
            Self::Archive => Slot::Archive,
            Self::AddLabel { .. } => Slot::Label,
            Self::MarkRead => Slot::Read,
            Self::NeedsReply { .. } => Slot::NeedsReply,
            Self::AsksAction { .. } => Slot::AsksAction,
            Self::Surface { .. } => Slot::Surface,
            Self::Trash | Self::Spam => Slot::Bin,
            Self::MarkPaid => Slot::Money,
            Self::ShowImages => Slot::Images,
        }
    }
}

/// Why a rule is not valid.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Invalid {
    /// No condition, or more than [`MAX_CONDITIONS`].
    #[error("a rule needs one to four conditions")]
    Conditions,
    /// No action, or more than [`MAX_ACTIONS`].
    #[error("a rule needs one to four actions")]
    Actions,
    /// Two actions fill the same slot.
    #[error("two actions do the same kind of thing")]
    SameSlot,
    /// A value is empty or a stream is unknown.
    #[error("a condition or action has no usable value")]
    Value,
    /// A domain of a free mail provider, on its own.
    #[error("a free mail domain alone would match people")]
    FreeMailDomain,
    /// Neither the sender, a list, a header nor the subject is tested.
    #[error("a rule must say which mail it is about")]
    TooBroad,
    /// The trash or spam for a whole domain: people the inbox writes to may
    /// send from it.
    #[error("a whole domain is never put in the trash or spam")]
    DomainToBin,
    /// Showing images is a rule of its own about one sender: one address or
    /// one domain, and nothing else.
    #[error("showing images needs one sender, and nothing else")]
    ImagesNeedSender,
    /// Trusting a whole domain anyone can send from (a mailing list or a
    /// newsletter platform) with images.
    #[error("anyone can send from this domain: trust an address instead")]
    SharedSenderDomain,
}

/// Free mail providers, and the mail of internet providers, by registrable
/// domain: a domain of theirs on its own would match people (anyone with an
/// account there). The one list: the web client's copy is checked against it
/// (`clients/app/src/lib/mail/free-mail.ts`, campbooks-cloud#235).
pub const FREE_MAIL: &[&str] = &[
    "10minutemail.com",
    "126.com",
    "139.com",
    "163.com",
    "189.cn",
    "abv.bg",
    "aim.com",
    "alice.it",
    "aliyun.com",
    "aol.co.uk",
    "aol.com",
    "aol.de",
    "aol.fr",
    "aon.at",
    "arcor.de",
    "atlas.cz",
    "att.net",
    "azet.sk",
    "bbox.fr",
    "bell.net",
    "bellsouth.net",
    "biglobe.ne.jp",
    "bigmir.net",
    "bigpond.com",
    "bigpond.net.au",
    "bk.ru",
    "bluewin.ch",
    "blueyonder.co.uk",
    "bol.com.br",
    "bredband.net",
    "btinternet.com",
    "btopenworld.com",
    "centrum.cz",
    "centrum.sk",
    "centurylink.net",
    "charter.net",
    "chello.at",
    "chello.nl",
    "citromail.hu",
    "clix.pt",
    "club-internet.fr",
    "cogeco.ca",
    "comcast.net",
    "comhem.se",
    "cox.net",
    "daum.net",
    "dir.bg",
    "disroot.org",
    "docomo.ne.jp",
    "duck.com",
    "earthlink.net",
    "eircom.net",
    "email.com",
    "email.cz",
    "email.it",
    "embarqmail.com",
    "excite.com",
    "ezweb.ne.jp",
    "fastmail.com",
    "fastmail.fm",
    "fastwebnet.it",
    "foxmail.com",
    "free.fr",
    "freemail.hu",
    "freenet.de",
    "frontier.com",
    "frontiernet.net",
    "gazeta.pl",
    "globo.com",
    "gmail.com",
    "gmx.at",
    "gmx.ch",
    "gmx.co.uk",
    "gmx.com",
    "gmx.de",
    "gmx.es",
    "gmx.fr",
    "gmx.it",
    "gmx.net",
    "gmx.pt",
    "googlemail.com",
    "guerrillamail.com",
    "hanmail.net",
    "hetnet.nl",
    "hey.com",
    "hispeed.ch",
    "home.nl",
    "hotmail.be",
    "hotmail.ca",
    "hotmail.ch",
    "hotmail.cl",
    "hotmail.co.jp",
    "hotmail.co.uk",
    "hotmail.com",
    "hotmail.com.ar",
    "hotmail.com.au",
    "hotmail.com.br",
    "hotmail.com.mx",
    "hotmail.com.tr",
    "hotmail.de",
    "hotmail.dk",
    "hotmail.es",
    "hotmail.fi",
    "hotmail.fr",
    "hotmail.gr",
    "hotmail.it",
    "hotmail.nl",
    "hotmail.no",
    "hotmail.se",
    "hushmail.com",
    "i.ua",
    "icloud.com",
    "ig.com.br",
    "iinet.net.au",
    "inbox.com",
    "inbox.lt",
    "inbox.lv",
    "inbox.ru",
    "interia.pl",
    "iol.pt",
    "juno.com",
    "kakao.com",
    "kpnmail.nl",
    "laposte.net",
    "libero.it",
    "list.ru",
    "live.be",
    "live.ca",
    "live.cl",
    "live.co.uk",
    "live.com",
    "live.com.ar",
    "live.com.au",
    "live.com.mx",
    "live.com.pt",
    "live.de",
    "live.dk",
    "live.fr",
    "live.ie",
    "live.in",
    "live.it",
    "live.jp",
    "live.nl",
    "live.no",
    "live.ru",
    "live.se",
    "luukku.com",
    "lycos.com",
    "mac.com",
    "mail.bg",
    "mail.com",
    "mail.de",
    "mail.ee",
    "mail.ru",
    "mailbox.org",
    "mailfence.com",
    "mailinator.com",
    "me.com",
    "meta.ua",
    "mindspring.com",
    "movistar.es",
    "msn.com",
    "mweb.co.za",
    "nate.com",
    "naver.com",
    "netcabo.pt",
    "netzero.net",
    "neuf.fr",
    "nifty.com",
    "ntlworld.com",
    "numericable.fr",
    "o2.pl",
    "ocn.ne.jp",
    "onet.pl",
    "online.de",
    "online.no",
    "op.pl",
    "optonline.net",
    "optusnet.com.au",
    "orange.fr",
    "orcon.net.nz",
    "outlook.at",
    "outlook.be",
    "outlook.cl",
    "outlook.co.id",
    "outlook.co.il",
    "outlook.co.nz",
    "outlook.co.th",
    "outlook.com",
    "outlook.com.ar",
    "outlook.com.au",
    "outlook.com.br",
    "outlook.com.gr",
    "outlook.com.tr",
    "outlook.com.vn",
    "outlook.cz",
    "outlook.de",
    "outlook.dk",
    "outlook.es",
    "outlook.fr",
    "outlook.hu",
    "outlook.ie",
    "outlook.in",
    "outlook.it",
    "outlook.jp",
    "outlook.kr",
    "outlook.lv",
    "outlook.my",
    "outlook.ph",
    "outlook.pt",
    "outlook.sa",
    "outlook.sg",
    "outlook.sk",
    "passport.com",
    "planet.nl",
    "pm.me",
    "post.cz",
    "posteo.de",
    "posteo.net",
    "prodigy.net.mx",
    "proton.me",
    "protonmail.ch",
    "protonmail.com",
    "proximus.be",
    "q.com",
    "qq.com",
    "rambler.ru",
    "rediffmail.com",
    "riseup.net",
    "roadrunner.com",
    "rocketmail.com",
    "rogers.com",
    "rr.com",
    "runbox.com",
    "sapo.pt",
    "sbcglobal.net",
    "scarlet.be",
    "seznam.cz",
    "sfr.fr",
    "sharklasers.com",
    "shaw.ca",
    "sina.cn",
    "sina.com",
    "sky.com",
    "skynet.be",
    "so-net.ne.jp",
    "softbank.ne.jp",
    "sohu.com",
    "spray.se",
    "startmail.com",
    "sunrise.ch",
    "suomi24.fi",
    "sympatico.ca",
    "t-online.de",
    "talktalk.net",
    "telefonica.net",
    "telenet.be",
    "telia.com",
    "telkomsa.net",
    "telus.net",
    "temp-mail.org",
    "terra.com.br",
    "terra.es",
    "tin.it",
    "tiscali.co.uk",
    "tiscali.it",
    "tlen.pl",
    "tpg.com.au",
    "trashmail.com",
    "tuta.com",
    "tuta.io",
    "tutamail.com",
    "tutanota.com",
    "tutanota.de",
    "ukr.net",
    "uol.com.br",
    "usa.com",
    "verizon.net",
    "videotron.ca",
    "virgilio.it",
    "virginmedia.com",
    "vodamail.co.za",
    "volny.cz",
    "voo.be",
    "walla.co.il",
    "wanadoo.fr",
    "web.de",
    "webmail.co.za",
    "windowslive.com",
    "windstream.net",
    "wp.pl",
    "xfinity.com",
    "xs4all.nl",
    "xtra.co.nz",
    "ya.ru",
    "yahoo.at",
    "yahoo.be",
    "yahoo.ca",
    "yahoo.ch",
    "yahoo.cn",
    "yahoo.co.id",
    "yahoo.co.in",
    "yahoo.co.jp",
    "yahoo.co.kr",
    "yahoo.co.nz",
    "yahoo.co.th",
    "yahoo.co.uk",
    "yahoo.com",
    "yahoo.com.ar",
    "yahoo.com.au",
    "yahoo.com.br",
    "yahoo.com.hk",
    "yahoo.com.mx",
    "yahoo.com.my",
    "yahoo.com.ph",
    "yahoo.com.sg",
    "yahoo.com.tr",
    "yahoo.com.tw",
    "yahoo.com.vn",
    "yahoo.de",
    "yahoo.dk",
    "yahoo.es",
    "yahoo.fi",
    "yahoo.fr",
    "yahoo.gr",
    "yahoo.ie",
    "yahoo.in",
    "yahoo.it",
    "yahoo.nl",
    "yahoo.no",
    "yahoo.pl",
    "yahoo.pt",
    "yahoo.ro",
    "yahoo.se",
    "yandex.by",
    "yandex.com",
    "yandex.com.tr",
    "yandex.kz",
    "yandex.ru",
    "yandex.ua",
    "yeah.net",
    "ymail.com",
    "yopmail.com",
    "ziggo.nl",
    "zoho.com",
    "zohomail.com",
    "zohomail.eu",
    "zoznam.sk",
];

/// Whether `domain` is a domain someone owns: two labels or more, and not a
/// public suffix (`co.uk`).
pub fn is_owned_domain(domain: &str) -> bool {
    let domain = domain.trim().trim_matches('.').to_ascii_lowercase();
    domain.split('.').filter(|l| !l.is_empty()).count() >= 2 && !is_public_suffix(&domain)
}

/// Platforms anyone can send from and still pass the domain's checks:
/// mailing lists, newsletter and mass mail services, help desks whose
/// customers send from their subdomains, and address relays (each relayed
/// sender is someone else). A rule may still sort their mail by domain, but
/// never trust a whole one with images (campbooks-cloud#235).
pub const SHARED_SENDERS: &[&str] = &[
    "addy.io",
    "amazonses.com",
    "anonaddy.com",
    "anonaddy.me",
    "beehiiv.com",
    "brevo.com",
    "buttondown.com",
    "buttondown.email",
    "campaign-archive.com",
    "ccsend.com",
    "constantcontact.com",
    "convertkit.com",
    "createsend.com",
    "freelists.org",
    "freshdesk.com",
    "ghost.io",
    "googlegroups.com",
    "groups.io",
    "hubspotemail.net",
    "intercom-mail.com",
    "kit.com",
    "list-manage.com",
    "mailchimp.com",
    "mailchimpapp.com",
    "mailerlite.com",
    "mailgun.org",
    "mandrillapp.com",
    "mcdlv.net",
    "mcsv.net",
    "medium.com",
    "mozmail.com",
    "patreon.com",
    "privaterelay.appleid.com",
    "rsgsv.net",
    "sendgrid.net",
    "sendinblue.com",
    "simplelogin.com",
    "sourceforge.net",
    "sparkpostmail.com",
    "substack.com",
    "yahoogroups.com",
    "zendesk.com",
];

/// Whether trusting anyone at `domain` with images would trust strangers: a
/// free mail or internet provider's domain, a platform anyone can send from,
/// or a public suffix. Checked when an image rule is written and again
/// whenever one is read, so a rule stored before a domain joined a list
/// never acts (campbooks-cloud#235).
pub fn images_domain_too_broad(domain: &str) -> bool {
    !is_owned_domain(domain) || is_free_mail(domain) || is_shared_sender(domain)
}

/// Whether `domain` reaches a platform anyone can send from
/// ([`SHARED_SENDERS`]): it is one, is under one, or is above one (a rule for
/// anyone at `appleid.com` reaches `privaterelay.appleid.com`).
pub fn is_shared_sender(domain: &str) -> bool {
    let domain = domain.trim().trim_matches('.').to_ascii_lowercase();
    !domain.is_empty()
        && SHARED_SENDERS.iter().any(|shared| {
            domain == *shared
                || domain.ends_with(&format!(".{shared}"))
                || shared.ends_with(&format!(".{domain}"))
        })
}

/// Whether `domain` (or the registrable domain it is under) belongs to a
/// free mail provider.
pub fn is_free_mail(domain: &str) -> bool {
    let domain = domain.trim().trim_matches('.').to_ascii_lowercase();
    // By registrable domain, so `mail.yahoo.fr` is Yahoo's too.
    FREE_MAIL.contains(&domain.as_str())
        || FREE_MAIL.contains(&registrable_domain(&domain).as_str())
}

/// One taught rule, as the engine sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaughtRule {
    /// Stable id (the owning module's), used last to break ties.
    pub id: String,
    /// All must hold.
    pub conditions: Vec<Condition>,
    /// What it does.
    pub actions: Vec<Action>,
    /// When a member confirmed it (unix milliseconds); later wins ties.
    pub confirmed_at: i64,
}

impl TaughtRule {
    /// The sum of its conditions' weights.
    pub fn specificity(&self) -> u32 {
        self.conditions.iter().map(Condition::weight).sum()
    }

    /// Checks the shape of a rule before it is stored.
    ///
    /// # Errors
    /// The first problem found.
    pub fn validate(conditions: &[Condition], actions: &[Action]) -> Result<(), Invalid> {
        if conditions.is_empty() || conditions.len() > MAX_CONDITIONS {
            return Err(Invalid::Conditions);
        }
        if actions.is_empty() || actions.len() > MAX_ACTIONS {
            return Err(Invalid::Actions);
        }
        let mut slots = BTreeSet::new();
        for action in actions {
            if action.slot() != Slot::Label && !slots.insert(action.slot()) {
                return Err(Invalid::SameSlot);
            }
            let ok = match action {
                Action::MoveToStream { stream } => triage::STREAMS.contains(&stream.as_str()),
                Action::AddLabel { label } => !label.trim().is_empty(),
                _ => true,
            };
            if !ok {
                return Err(Invalid::Value);
            }
        }
        for condition in conditions {
            let ok = match condition {
                Condition::SenderAddress(v) => v.contains('@') && !v.trim().is_empty(),
                Condition::SenderDomain(v) => !v.contains('@') && is_owned_domain(v),
                Condition::ListId(v) | Condition::SubjectTemplate(v) | Condition::Label(v) => {
                    !v.trim().is_empty()
                }
                Condition::SubjectContains(v) => !subject_words(v).is_empty(),
                Condition::Header(test) => test.value.as_ref().is_none_or(|v| !v.trim().is_empty()),
                Condition::Stream(v) => triage::STREAMS.contains(&v.as_str()),
                Condition::SenderChecksFail => true,
            };
            if !ok {
                return Err(Invalid::Value);
            }
        }
        let narrows = |c: &Condition| {
            matches!(
                c,
                Condition::SenderAddress(_)
                    | Condition::ListId(_)
                    | Condition::SubjectTemplate(_)
                    | Condition::SubjectContains(_)
                    | Condition::Header(_)
            )
        };
        let domains: Vec<&String> = conditions
            .iter()
            .filter_map(|c| match c {
                Condition::SenderDomain(d) => Some(d),
                _ => None,
            })
            .collect();
        if !conditions.iter().any(narrows) {
            if domains.is_empty() {
                return Err(Invalid::TooBroad);
            }
            if domains.iter().any(|d| is_free_mail(&registrable_domain(d))) {
                return Err(Invalid::FreeMailDomain);
            }
        }
        // The trash and spam need more than a domain: a narrowing condition,
        // or "fails its sender checks" (mail pretending to be it).
        let bins = actions
            .iter()
            .any(|a| matches!(a, Action::Trash | Action::Spam));
        if bins
            && !conditions
                .iter()
                .any(|c| narrows(c) || *c == Condition::SenderChecksFail)
        {
            return Err(Invalid::DomainToBin);
        }
        // Trusting a sender's images: one sender (an address, or a domain
        // the checks above allow), as the rule's only condition and action.
        if actions.contains(&Action::ShowImages)
            && (actions.len() != 1
                || !matches!(
                    conditions,
                    [Condition::SenderAddress(_) | Condition::SenderDomain(_)]
                ))
        {
            return Err(Invalid::ImagesNeedSender);
        }
        if actions.contains(&Action::ShowImages)
            && conditions
                .iter()
                .any(|c| matches!(c, Condition::SenderDomain(d) if images_domain_too_broad(d)))
        {
            return Err(Invalid::SharedSenderDomain);
        }
        Ok(())
    }

    /// Whether every condition holds for `facts`.
    pub fn matches(&self, facts: &Facts<'_>) -> bool {
        !self.conditions.is_empty() && self.conditions.iter().all(|c| c.holds(facts))
    }

    /// Whether this rule shows images for anyone at a domain that would
    /// trust strangers ([`images_domain_too_broad`]): it never acts.
    pub fn images_too_broad(&self) -> bool {
        self.actions.contains(&Action::ShowImages)
            && self
                .conditions
                .iter()
                .any(|c| matches!(c, Condition::SenderDomain(d) if images_domain_too_broad(d)))
    }

    /// Precedence: more specific first, then later confirmed, then smaller id.
    fn precedes(&self, other: &Self) -> Ordering {
        other
            .specificity()
            .cmp(&self.specificity())
            .then(other.confirmed_at.cmp(&self.confirmed_at))
            .then(self.id.cmp(&other.id))
    }
}

/// What the engine reads of one message and its conversation.
#[derive(Debug, Clone, Default)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "independent facts about one message, each read by its own condition or guard"
)]
pub struct Facts<'a> {
    /// The sender's address.
    pub sender_address: &'a str,
    /// The subject.
    pub subject: &'a str,
    /// Its template, when it has one.
    pub subject_template: Option<&'a str>,
    /// `List-Id`.
    pub list_id: Option<&'a str>,
    /// `List-Unsubscribe`.
    pub list_unsubscribe: Option<&'a str>,
    /// `Precedence`.
    pub precedence: Option<&'a str>,
    /// `Auto-Submitted`.
    pub auto_submitted: Option<&'a str>,
    /// The conversation's label ids.
    pub labels: &'a [String],
    /// The conversation's stream.
    pub stream: &'a str,
    /// The message looks suspicious, or its conversation has an open
    /// Review need.
    pub suspicious: bool,
    /// The receiving provider's authentication results say the sender's
    /// domain failed its checks (`EmailFacts::authentication_failed`): the
    /// broad test, which keeps `mark_paid` off such mail.
    pub auth_failed: bool,
    /// They hold an explicit DMARC failure under a published reject or
    /// quarantine policy (`signals::facts::dmarc_refused`): the narrow test
    /// "fails its sender checks" reads, since the domain's genuine mail
    /// passes.
    pub impersonated: bool,
    /// The inbox writes to the sender: "fails its sender checks" never
    /// matches their mail.
    pub correspondent: bool,
}

/// One slot's answer and the rule that gave it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Applied<T> {
    /// The value.
    pub value: T,
    /// The rule's id.
    pub rule: String,
}

/// What the matching rules say about one message.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Outcome {
    /// The stream to put it in.
    pub stream: Option<Applied<String>>,
    /// Archive it.
    pub archive: Option<String>,
    /// Mark it read.
    pub read: Option<String>,
    /// Labels to add, each with its rule.
    pub labels: Vec<Applied<String>>,
    /// "Needs a reply".
    pub needs_reply: Option<Applied<bool>>,
    /// "Asks for an action".
    pub asks_action: Option<Applied<bool>>,
    /// Always or never show.
    pub surface: Option<Applied<Surface>>,
    /// Into the trash or spam.
    pub bin: Option<Applied<Bin>>,
    /// Its bills are paid.
    pub paid: Option<String>,
}

impl Outcome {
    /// Whether no rule said anything.
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    /// Every rule that contributed, once each, in a stable order.
    pub fn rules(&self) -> BTreeSet<&str> {
        let mut out = BTreeSet::new();
        out.extend(self.stream.as_ref().map(|a| a.rule.as_str()));
        out.extend(self.archive.as_deref());
        out.extend(self.read.as_deref());
        out.extend(self.labels.iter().map(|a| a.rule.as_str()));
        out.extend(self.needs_reply.as_ref().map(|a| a.rule.as_str()));
        out.extend(self.asks_action.as_ref().map(|a| a.rule.as_str()));
        out.extend(self.surface.as_ref().map(|a| a.rule.as_str()));
        out.extend(self.bin.as_ref().map(|a| a.rule.as_str()));
        out.extend(self.paid.as_deref());
        out
    }
}

/// What the active `rules` say about a message with `facts`.
pub fn evaluate(rules: &[TaughtRule], facts: &Facts<'_>) -> Outcome {
    let mut matching: Vec<&TaughtRule> = rules.iter().filter(|r| r.matches(facts)).collect();
    matching.sort_by(|a, b| a.precedes(b));

    let mut out = Outcome::default();
    for rule in &matching {
        for action in &rule.actions {
            let id = || rule.id.clone();
            match action {
                Action::MoveToStream { stream } if out.stream.is_none() => {
                    out.stream = Some(Applied {
                        value: stream.clone(),
                        rule: id(),
                    });
                }
                Action::Archive if out.archive.is_none() => out.archive = Some(id()),
                Action::MarkRead if out.read.is_none() => out.read = Some(id()),
                Action::AddLabel { label } if !out.labels.iter().any(|l| &l.value == label) => {
                    out.labels.push(Applied {
                        value: label.clone(),
                        rule: id(),
                    });
                }
                Action::NeedsReply { value } if out.needs_reply.is_none() => {
                    out.needs_reply = Some(Applied {
                        value: *value,
                        rule: id(),
                    });
                }
                Action::AsksAction { value } if out.asks_action.is_none() => {
                    out.asks_action = Some(Applied {
                        value: *value,
                        rule: id(),
                    });
                }
                Action::Surface { value } if out.surface.is_none() => {
                    out.surface = Some(Applied {
                        value: *value,
                        rule: id(),
                    });
                }
                Action::Trash | Action::Spam if out.bin.is_none() => {
                    let value = if *action == Action::Trash {
                        Bin::Trash
                    } else {
                        Bin::Spam
                    };
                    out.bin = Some(Applied { value, rule: id() });
                }
                Action::MarkPaid if out.paid.is_none() => out.paid = Some(id()),
                _ => {}
            }
        }
    }

    let always = out
        .surface
        .as_ref()
        .is_some_and(|s| s.value == Surface::Always);
    if always {
        // "Never hide this" outranks the trash and spam too.
        out.bin = None;
    }
    let keep_visible = facts.suspicious || always;
    if keep_visible {
        out.archive = None;
        out.read = None;
        if out.stream.as_ref().is_some_and(|s| s.value != "primary") {
            out.stream = None;
        }
    }
    if facts.suspicious {
        if out.needs_reply.as_ref().is_some_and(|a| !a.value) {
            out.needs_reply = None;
        }
        if out.asks_action.as_ref().is_some_and(|a| !a.value) {
            out.asks_action = None;
        }
        if out
            .surface
            .as_ref()
            .is_some_and(|s| s.value == Surface::Never)
        {
            out.surface = None;
        }
        out.paid = None;
    }
    // A bill whose sender failed its checks may be a fake one: never marked
    // paid without the member, whatever the model made of it.
    if facts.auth_failed {
        out.paid = None;
    }
    out
}

/// The rule that shows the remote images of a message with `facts`, if one
/// does (campbooks-cloud#200): an active rule with [`Action::ShowImages`]
/// whose sender matches, the most specific first. Only when the receiving
/// provider's results `authenticated` the From domain (a DMARC pass for it,
/// or an aligned DKIM pass; no results at all is not a pass), so a spoofed
/// sender never gets its images shown; never for mail that looks
/// suspicious or failed its checks; and never by a rule for anyone at a
/// domain that would trust strangers, however old ([`TaughtRule::images_too_broad`]).
pub fn shows_images<'r>(
    rules: &'r [TaughtRule],
    facts: &Facts<'_>,
    authenticated: bool,
) -> Option<&'r str> {
    if !authenticated || facts.suspicious || facts.auth_failed || facts.impersonated {
        return None;
    }
    let mut matching: Vec<&TaughtRule> = rules
        .iter()
        .filter(|r| {
            r.actions.contains(&Action::ShowImages) && !r.images_too_broad() && r.matches(facts)
        })
        .collect();
    matching.sort_by(|a, b| a.precedes(b));
    matching.first().map(|r| r.id.as_str())
}

/// A `List-Id` as rules compare it: lower-cased, the id inside its angle
/// brackets (`Design Notes <digest.design-notes.example>` →
/// `digest.design-notes.example`).
pub fn normalise_list_id(value: &str) -> String {
    let value = value.trim().to_ascii_lowercase();
    match (value.rfind('<'), value.rfind('>')) {
        (Some(start), Some(end)) if start < end => value[start + 1..end].trim().to_owned(),
        _ => value,
    }
}

fn contains_words(subject: &str, phrase: &str) -> bool {
    let want = subject_words(phrase);
    if want.is_empty() {
        return false;
    }
    let have = subject_words(subject);
    have.windows(want.len()).any(|w| w == want.as_slice())
}

/// The active rules of one inbox as a [`Strategy`] for the `stream`,
/// `needs_reply` and `asks_action` questions, reading the envelope alone (a
/// new conversation: no labels, Primary, not suspicious). It abstains on every
/// other question and where no rule matches.
pub struct TaughtRules {
    rules: Vec<TaughtRule>,
}

impl TaughtRules {
    /// A strategy over `rules`.
    pub fn new(rules: Vec<TaughtRule>) -> Self {
        Self { rules }
    }
}

/// The rule id a taught answer's [`Reason`] names.
pub const REASON: &str = "taught.rule";

impl Strategy for TaughtRules {
    fn name(&self) -> &'static str {
        "taught"
    }

    fn decide<'a>(
        &'a self,
        envelope: &'a Envelope,
        questions: &'a [Question],
    ) -> BoxFuture<'a, Result<Decisions, Error>> {
        Box::pin(async move {
            let started = Instant::now();
            let template = super::signals::email::subject_template(&envelope.subject);
            let facts = Facts {
                sender_address: &envelope.from.address,
                subject: &envelope.subject,
                subject_template: template.as_deref(),
                list_id: envelope.header("list-id"),
                list_unsubscribe: envelope.header("list-unsubscribe"),
                precedence: envelope.header("precedence"),
                auto_submitted: envelope.header("auto-submitted"),
                labels: &[],
                stream: "primary",
                suspicious: false,
                auth_failed: false,
                impersonated: false,
                correspondent: false,
            };
            let outcome = evaluate(&self.rules, &facts);
            let answers = questions
                .iter()
                .map(|q| {
                    let (index, rule) = match (q.id.as_str(), &q.kind) {
                        (triage::STREAM, QuestionKind::Choice { options, .. }) => {
                            let applied = outcome.stream.as_ref()?;
                            let index = options.iter().position(|o| o.label == applied.value)?;
                            (index, applied.rule.clone())
                        }
                        (triage::NEEDS_REPLY, _) => {
                            let a = outcome.needs_reply.as_ref()?;
                            (usize::from(a.value), a.rule.clone())
                        }
                        (triage::ASKS_ACTION, _) => {
                            let a = outcome.asks_action.as_ref()?;
                            (usize::from(a.value), a.rule.clone())
                        }
                        _ => return None,
                    };
                    Some(Decided {
                        answer: local_answer(
                            q,
                            peaked(q.option_count(), index, 1.0),
                            "taught",
                            started,
                        ),
                        reasons: vec![Reason::new(REASON, rule)],
                    })
                })
                .collect();
            Ok(Decisions {
                answers,
                remote_calls: 0,
            })
        })
    }
}

#[cfg(test)]
mod tests;
