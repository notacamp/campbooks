//! What Scout notices about one sender's mail, from facts and counts only:
//! how often and how regularly it comes (addresses and dates), what it is
//! (the stream the decide pass sorted it to, whether a payment is owed,
//! whether it looked suspicious) and what the inbox does with it (opened,
//! archived or deleted unread, replied). No body or attachment is read, and
//! no phrase list: what the mail says is the model's, and the stream it
//! chose is all this uses of it (ADR 0018).
//!
//! A sender's mail may hold more than one kind (campbooks-cloud#356): a shop
//! sends promotions the member ignores and shipping notices they read. Its
//! conversations are split by their subject template, compared for equality
//! only (text hygiene, the same likeness a member's rule names), and each
//! kind is counted on its own; a kind can also be words the `complete` model
//! proposed for the sender's subjects ([`read`]'s `phrases`), counted with
//! the rule engine's own `SubjectContains` test. What a kind is never comes
//! from code reading the subject: it is equal templates, or the model's
//! words, and the member confirms the rule.

use std::collections::{BTreeMap, BTreeSet};

use campbooks_ai::decide::taught::contains_words;
use chrono::{DateTime, TimeDelta, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    config::NoticeSettings,
    domain::rules::{RuleAction, RuleCondition, SurfaceWhen},
    model::{NoticeKind, Stream},
};

/// Longest example subject kept, in characters.
pub const EXAMPLE_CHARS: usize = 200;
/// The share of a mixed sender's conversations, in percent, that fits no
/// template kind before the model is asked to group its subjects.
const UNSORTED_PERCENT_MIN: usize = 30;

/// One sender's mail in an inbox over the window, as counted.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "independent yes/no facts about a sender's mail, each a reason not to offer"
)]
pub struct SenderStats {
    /// The sender's address, lower-cased.
    pub sender: String,
    /// Conversations whose newest inbound message is theirs.
    pub received: u32,
    /// Of those, read (opened, here or at the provider).
    pub opened: u32,
    /// Of those, archived or deleted while still unread.
    pub archived_unread: u32,
    /// Of those, sent in the last seven days.
    pub last_week: u32,
    /// The distinct days they were sent on.
    pub days: u32,
    /// The inbox wrote in one of those conversations.
    pub replied: bool,
    /// One of them looked suspicious.
    pub suspicious: bool,
    /// One of them asks for a payment still owed (the money question).
    pub owed: bool,
    /// One of them needed a reply or an action the member did not dismiss.
    pub asked: bool,
    /// The stream most of them are in.
    pub stream: Stream,
    /// The headers say a machine sent it (list mail, automatic, no-reply).
    pub machine: bool,
    /// The inbox writes to this address.
    pub correspondent: bool,
    /// When each was sent, oldest first.
    pub dates: Vec<DateTime<Utc>>,
}

/// One conversation of a sender's in the window: what the inbox did with
/// it, the facts about it, and its subject, kept only to group it by its
/// template (equality) or the model's words and to show one as an example.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "independent yes/no facts about one conversation, read straight from SQL"
)]
pub struct Mail {
    /// The subject of its newest inbound message.
    pub subject: String,
    /// That subject's template, when it has one.
    pub template: Option<String>,
    /// When that message was sent.
    pub sent_at: DateTime<Utc>,
    /// Read, here or at the provider.
    pub opened: bool,
    /// Archived or deleted while still unread.
    pub archived_unread: bool,
    /// The inbox wrote in it.
    pub replied: bool,
    /// It looked suspicious.
    pub suspicious: bool,
    /// It asks for a payment still owed.
    pub owed: bool,
    /// It needed a reply or an action the member did not dismiss.
    pub asked: bool,
    /// Its stream.
    pub stream: Stream,
}

/// A sender and its mail in the window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sender {
    /// The address, lower-cased.
    pub address: String,
    /// The headers of its newest message say a machine sent it.
    pub machine: bool,
    /// The inbox writes to this address.
    pub correspondent: bool,
    /// Its conversations, newest first.
    pub mail: Vec<Mail>,
}

impl SenderStats {
    /// The counts of `ids` (indexes into `sender.mail`) as of `now`.
    pub fn of(sender: &Sender, ids: &BTreeSet<usize>, now: DateTime<Utc>) -> Self {
        let mail: Vec<&Mail> = ids.iter().filter_map(|&i| sender.mail.get(i)).collect();
        let count = |f: fn(&Mail) -> bool| {
            u32::try_from(mail.iter().filter(|m| f(m)).count()).unwrap_or(u32::MAX)
        };
        let week_ago = now - TimeDelta::days(7);
        let mut dates: Vec<DateTime<Utc>> = mail.iter().map(|m| m.sent_at).collect();
        dates.sort_unstable();
        // The stream most of them are in; of two as common, the first by
        // name (as SQL's `mode()` picks it).
        let mut streams: BTreeMap<&'static str, (u32, Stream)> = BTreeMap::new();
        for m in &mail {
            streams.entry(m.stream.as_str()).or_insert((0, m.stream)).0 += 1;
        }
        let stream = streams
            .values()
            .fold(None::<(u32, Stream)>, |best, &(n, s)| match best {
                Some((m, _)) if m >= n => best,
                _ => Some((n, s)),
            })
            .map_or(Stream::Primary, |(_, s)| s);
        Self {
            sender: sender.address.clone(),
            received: count(|_| true),
            opened: count(|m| m.opened),
            archived_unread: count(|m| m.archived_unread),
            last_week: u32::try_from(mail.iter().filter(|m| m.sent_at > week_ago).count())
                .unwrap_or(u32::MAX),
            days: distinct_days(&mail),
            replied: mail.iter().any(|m| m.replied),
            suspicious: mail.iter().any(|m| m.suspicious),
            owed: mail.iter().any(|m| m.owed),
            asked: mail.iter().any(|m| m.asked),
            stream,
            machine: sender.machine,
            correspondent: sender.correspondent,
            dates,
        }
    }
}

fn distinct_days(mail: &[&Mail]) -> u32 {
    let days: BTreeSet<_> = mail.iter().map(|m| m.sent_at.date_naive()).collect();
    u32::try_from(days.len()).unwrap_or(u32::MAX)
}

/// How regularly a sender writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Cadence {
    /// About once a day or more.
    Daily,
    /// About once a week.
    Weekly,
    /// About once a month.
    Monthly,
}

/// What a suggestion says, in numbers the client turns into a sentence
/// ("You got 34 emails from builds@ci.example.com in the last 60 days and
/// opened 2").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct NoticeEvidence {
    /// Conversations from the sender in the window.
    pub received: u32,
    /// Of those, opened.
    pub opened: u32,
    /// Of those, archived or deleted unread.
    pub archived_unread: u32,
    /// Of those, sent in the last seven days.
    pub last_week: u32,
    /// The window, in days.
    pub window_days: u32,
    /// How regularly they come, when they do.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cadence: Option<Cadence>,
}

/// Which part of a sender's mail a suggestion is about.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Pattern {
    /// All of it.
    Sender,
    /// The mail whose subject has this template.
    Template(String),
    /// The mail whose subject holds these words (lower-cased, accents
    /// dropped), as the model proposed them and the pass checked them.
    Phrase(String),
}

impl Pattern {
    /// As stored (`rule_notices.pattern`): `''`, `t:<template>` or
    /// `p:<words>`.
    pub fn key(&self) -> String {
        match self {
            Self::Sender => String::new(),
            Self::Template(template) => format!("t:{template}"),
            Self::Phrase(words) => format!("p:{words}"),
        }
    }

    /// Read back from [`Pattern::key`]; anything else is the whole sender.
    pub fn of_key(key: &str) -> Self {
        if let Some(template) = key.strip_prefix("t:") {
            Self::Template(template.to_owned())
        } else if let Some(words) = key.strip_prefix("p:") {
            Self::Phrase(words.to_owned())
        } else {
            Self::Sender
        }
    }

    /// The rule's conditions for `sender`'s mail of this kind.
    pub fn conditions(&self, sender: &str) -> Vec<RuleCondition> {
        let from = RuleCondition::SenderAddress(sender.to_owned());
        match self {
            Self::Sender => vec![from],
            Self::Template(template) => {
                vec![from, RuleCondition::SubjectTemplate(template.clone())]
            }
            Self::Phrase(words) => vec![from, RuleCondition::SubjectContains(words.clone())],
        }
    }
}

/// A rule Scout would offer for a sender, or for one kind of its mail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Noticed {
    /// What it noticed.
    pub kind: NoticeKind,
    /// Which part of the sender's mail.
    pub pattern: Pattern,
    /// One recent subject of that part (none for the whole sender).
    pub example: Option<String>,
    /// The rule's conditions.
    pub conditions: Vec<RuleCondition>,
    /// Its actions.
    pub actions: Vec<RuleAction>,
    /// Why, in numbers.
    pub evidence: NoticeEvidence,
}

/// How regularly mail sent at `dates` (oldest first) comes: the median gap
/// between two of them.
pub fn cadence(dates: &[DateTime<Utc>]) -> Option<Cadence> {
    let mut gaps: Vec<i64> = dates
        .windows(2)
        .map(|pair| (pair[1] - pair[0]).num_hours())
        .collect();
    if gaps.len() < 2 {
        return None;
    }
    gaps.sort_unstable();
    let median = gaps[gaps.len() / 2];
    match median {
        0..=36 => Some(Cadence::Daily),
        120..=216 => Some(Cadence::Weekly),
        600..=840 => Some(Cadence::Monthly),
        _ => None,
    }
}

/// What to offer for mail counted as `stats`, whatever its size: nothing for
/// a person the inbox writes to, for mail that looked suspicious, asked
/// something of the member or asks for a payment, nor for mail the member
/// answers or reads.
fn verdict(
    stats: &SenderStats,
    settings: &NoticeSettings,
) -> Option<(NoticeKind, Vec<RuleAction>)> {
    if stats.replied
        || stats.suspicious
        || stats.asked
        || stats.owed
        || stats.correspondent
        || !stats.machine
    {
        return None;
    }
    // Exact percentages (n / received against p / 100), never rounded down.
    let received = u64::from(stats.received.max(1));
    let at_least = |n: u32, p: u32| u64::from(n) * 100 >= u64::from(p) * received;
    let at_most = |n: u32, p: u32| u64::from(n) * 100 <= u64::from(p) * received;
    if at_least(stats.archived_unread, settings.archived_unread_percent_min) {
        return Some((
            NoticeKind::ArchiveOnArrival,
            vec![
                RuleAction::Archive,
                RuleAction::Surface {
                    value: SurfaceWhen::Never,
                },
            ],
        ));
    }
    if at_most(stats.opened, settings.opened_percent_max) {
        // Filed where the decide pass sorts most of it; mail it keeps in
        // Primary goes to Updates.
        let stream = if stats.stream == Stream::Primary {
            Stream::Updates
        } else {
            stats.stream
        };
        return Some((
            NoticeKind::KeepOffHome,
            vec![
                RuleAction::MoveToStream { stream },
                RuleAction::Surface {
                    value: SurfaceWhen::Never,
                },
            ],
        ));
    }
    None
}

/// What `stats` would be offered as `pattern`, with `example`.
fn noticed(
    stats: &SenderStats,
    pattern: Pattern,
    example: Option<String>,
    settings: &NoticeSettings,
) -> Option<Noticed> {
    let (kind, actions) = verdict(stats, settings)?;
    Some(Noticed {
        kind,
        conditions: pattern.conditions(&stats.sender),
        pattern,
        example,
        actions,
        evidence: NoticeEvidence {
            received: stats.received,
            opened: stats.opened,
            archived_unread: stats.archived_unread,
            last_week: stats.last_week,
            window_days: settings.window_days,
            cadence: cadence(&stats.dates),
        },
    })
}

/// The rule to offer for all of a sender's mail counted as `stats`, if any,
/// under `settings`: mail that keeps coming, and that matters to nobody
/// ([`verdict`]).
pub fn notice(stats: &SenderStats, settings: &NoticeSettings) -> Option<Noticed> {
    if stats.received < settings.min_messages || stats.days < settings.min_days {
        return None;
    }
    noticed(stats, Pattern::Sender, None, settings)
}

/// The rule to offer for one kind of a sender's mail counted as `stats`,
/// with `example`: as [`notice`], with the smaller size a kind needs.
fn kind_notice(
    stats: &SenderStats,
    pattern: Pattern,
    example: Option<String>,
    settings: &NoticeSettings,
) -> Option<Noticed> {
    if stats.received < settings.min_kind_messages || stats.days < settings.min_kind_days {
        return None;
    }
    noticed(stats, pattern, example, settings)
}

/// What the pass found for one sender.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Reading {
    /// What to offer, the most mail first: one rule for the whole sender, or
    /// one per kind of its mail that would be, or nothing.
    pub notices: Vec<Noticed>,
    /// The sender's mail is mixed and much of it fits no template: the
    /// model's grouping of its subjects may find kinds in it.
    pub to_group: bool,
}

/// The sender's subject template groups big enough to be kinds (most mail
/// first), and the conversations left over: those with no template and
/// those of smaller groups.
fn templates(
    sender: &Sender,
    settings: &NoticeSettings,
) -> (Vec<(String, BTreeSet<usize>)>, BTreeSet<usize>) {
    let mut groups: BTreeMap<&str, BTreeSet<usize>> = BTreeMap::new();
    let mut rest = BTreeSet::new();
    for (i, mail) in sender.mail.iter().enumerate() {
        match &mail.template {
            Some(template) => {
                groups.entry(template.as_str()).or_default().insert(i);
            }
            None => {
                rest.insert(i);
            }
        }
    }
    let mut kinds = Vec::new();
    for (template, ids) in groups {
        let mail: Vec<&Mail> = ids.iter().filter_map(|&i| sender.mail.get(i)).collect();
        if count(ids.len()) >= settings.min_kind_messages
            && distinct_days(&mail) >= settings.min_kind_days
        {
            kinds.push((template.to_owned(), ids));
        } else {
            rest.extend(ids);
        }
    }
    kinds.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then_with(|| a.0.cmp(&b.0)));
    (kinds, rest)
}

fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// The newest subject of `ids`, cut to [`EXAMPLE_CHARS`].
fn example(sender: &Sender, ids: &BTreeSet<usize>) -> Option<String> {
    // The mail is newest first: the smallest index is the newest.
    let newest = ids.first().and_then(|&i| sender.mail.get(i))?;
    Some(newest.subject.trim().chars().take(EXAMPLE_CHARS).collect())
}

/// The conversations of `sender` whose subject holds `phrase`, as the rule
/// engine matches it.
fn matching(sender: &Sender, phrase: &str) -> BTreeSet<usize> {
    sender
        .mail
        .iter()
        .enumerate()
        .filter(|(_, mail)| contains_words(&mail.subject, phrase))
        .map(|(i, _)| i)
        .collect()
}

/// Whether every kind of the sender's mail (each template kind, the mail
/// each of the model's `phrases` matches, and what is left, when there is
/// enough of each) would get what the whole of it gets, and none of it was
/// answered, asked anything or asks for a payment: then one rule for the
/// sender says it all.
fn all_one(
    sender: &Sender,
    whole: &SenderStats,
    (kinds, rest): (&[(String, BTreeSet<usize>)], &BTreeSet<usize>),
    phrases: &[String],
    settings: &NoticeSettings,
    now: DateTime<Utc>,
) -> bool {
    if whole.replied || whole.asked || whole.owed {
        return false;
    }
    let want = verdict(whole, settings).map(|(kind, _)| kind);
    let big = |ids: &BTreeSet<usize>| count(ids.len()) >= settings.min_kind_messages;
    let same = |ids: &BTreeSet<usize>| {
        verdict(&SenderStats::of(sender, ids, now), settings).map(|(kind, _)| kind) == want
    };
    kinds.iter().all(|(_, ids)| same(ids))
        && (!big(rest) || same(rest))
        && phrases.iter().all(|phrase| {
            let ids = matching(sender, phrase);
            !big(&ids) || same(&ids)
        })
}

/// Whether the mail left over is worth the model's grouping: enough of it,
/// a large enough share of the sender's, enough of it unread to make a kind,
/// and not plainly one with the rest (`one_offer`: the template kinds agree
/// with the whole sender, which would be offered a rule) while none of it
/// was read or mattered. Mail that is all one kind needs no grouping.
fn worth_grouping(
    sender: &Sender,
    rest: &BTreeSet<usize>,
    one_offer: bool,
    settings: &NoticeSettings,
) -> bool {
    let mail: Vec<&Mail> = rest.iter().filter_map(|&i| sender.mail.get(i)).collect();
    let unread = mail.iter().filter(|m| !m.opened).count();
    let mattered = mail
        .iter()
        .any(|m| m.opened || m.replied || m.asked || m.owed);
    count(rest.len()) >= settings.min_kind_messages
        && rest.len() * 100 >= sender.mail.len() * UNSORTED_PERCENT_MIN
        && count(unread) >= settings.min_kind_messages
        && (!one_offer || mattered)
}

/// Whether `sender`'s mail is all of one kind for what the inbox does with
/// it (teach once's default: the whole sender, or only mail like the one
/// acted on), its template kinds and the model's `phrases` for it counted.
/// A sender with no mail in the window is.
pub fn uniform(
    sender: &Sender,
    phrases: &[String],
    settings: &NoticeSettings,
    now: DateTime<Utc>,
) -> bool {
    if sender.mail.is_empty() {
        return true;
    }
    let all: BTreeSet<usize> = (0..sender.mail.len()).collect();
    let whole = SenderStats::of(sender, &all, now);
    let (kinds, rest) = templates(sender, settings);
    all_one(sender, &whole, (&kinds, &rest), phrases, settings, now)
}

/// What to offer for `sender`'s mail under `settings`, as of `now`, with
/// `phrases`, the model's grouping of its subjects if any:
///
/// 1. Nothing for a person, someone the inbox writes to, a sender with any
///    suspicious mail, or one that writes too little or in one burst.
/// 2. Its conversations are split by subject template; a template with
///    enough mail on enough days is a kind, counted on its own.
/// 3. When every kind would get what the whole sender gets and nothing of
///    it mattered, one rule for the sender; else one per kind that would be
///    offered (the sender and that template).
/// 4. For a mixed sender with much mail left over, each of the model's
///    phrases is a kind of the mail whose subject holds it, as the rule
///    engine matches it, when that is enough mail and mostly not offered
///    already (the sender and those words).
/// 5. A mixed sender with no kind to offer gets nothing.
pub fn read(
    sender: &Sender,
    phrases: &[String],
    settings: &NoticeSettings,
    now: DateTime<Utc>,
) -> Reading {
    let all: BTreeSet<usize> = (0..sender.mail.len()).collect();
    let whole = SenderStats::of(sender, &all, now);
    if whole.received < settings.min_messages
        || whole.days < settings.min_days
        || !whole.machine
        || whole.correspondent
        || whole.suspicious
    {
        return Reading::default();
    }
    let (kinds, rest) = templates(sender, settings);
    // Decided before the model's words are counted, so a pass knows whom to
    // ask about without them.
    let one_offer = notice(&whole, settings).is_some()
        && all_one(sender, &whole, (&kinds, &rest), &[], settings, now);
    let to_group = worth_grouping(sender, &rest, one_offer, settings);
    // One rule for all of it, when all of it is one kind and would be
    // offered; one kind that would be offered nothing still leaves the mail
    // left over to the model's words.
    if all_one(sender, &whole, (&kinds, &rest), phrases, settings, now)
        && let Some(noticed) = notice(&whole, settings)
    {
        return Reading {
            notices: vec![noticed],
            to_group,
        };
    }
    let mut notices = Vec::new();
    let mut offered: BTreeSet<usize> = BTreeSet::new();
    for (template, ids) in &kinds {
        let stats = SenderStats::of(sender, ids, now);
        let pattern = Pattern::Template(template.clone());
        if let Some(kind) = kind_notice(&stats, pattern, example(sender, ids), settings) {
            offered.extend(ids);
            notices.push(kind);
        }
    }
    if to_group {
        for phrase in phrases {
            // All the mail the rule would match, the template kinds' too:
            // what it covers is what is counted.
            let ids = matching(sender, phrase);
            if ids.is_empty() || ids.intersection(&offered).count() * 2 >= ids.len() {
                continue;
            }
            let stats = SenderStats::of(sender, &ids, now);
            let pattern = Pattern::Phrase(phrase.clone());
            if let Some(kind) = kind_notice(&stats, pattern, example(sender, &ids), settings) {
                offered.extend(&ids);
                notices.push(kind);
            }
        }
    }
    notices.sort_by_key(|n| std::cmp::Reverse(n.evidence.received));
    Reading { notices, to_group }
}

#[cfg(test)]
mod tests {
    use chrono::TimeDelta;

    use super::*;

    fn every(hours: i64, n: usize) -> Vec<DateTime<Utc>> {
        let start = DateTime::parse_from_rfc3339("2026-08-01T08:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        (0..n)
            .map(|i| start + TimeDelta::hours(hours * i64::try_from(i).unwrap()))
            .collect()
    }

    /// Build notifications: 34 a day apart, 2 opened, none archived.
    fn builds() -> SenderStats {
        SenderStats {
            sender: "builds@ci.example.com".into(),
            received: 34,
            opened: 2,
            archived_unread: 0,
            last_week: 7,
            days: 34,
            replied: false,
            suspicious: false,
            owed: false,
            asked: false,
            stream: Stream::Primary,
            machine: true,
            correspondent: false,
            dates: every(24, 34),
        }
    }

    #[test]
    fn mail_rarely_opened_is_kept_off_home_and_filed() {
        let settings = NoticeSettings::default();
        let noticed = notice(&builds(), &settings).unwrap();
        assert_eq!(noticed.kind, NoticeKind::KeepOffHome);
        assert_eq!(
            noticed.conditions,
            [RuleCondition::SenderAddress("builds@ci.example.com".into())]
        );
        assert_eq!(
            noticed.actions,
            [
                RuleAction::MoveToStream {
                    stream: Stream::Updates
                },
                RuleAction::Surface {
                    value: SurfaceWhen::Never
                }
            ]
        );
        assert_eq!(noticed.evidence.cadence, Some(Cadence::Daily));
        assert_eq!(noticed.evidence.window_days, 60);
        // Filed where the decide pass sorts it, when that is not Primary.
        let newsletters = SenderStats {
            stream: Stream::Newsletters,
            ..builds()
        };
        assert_eq!(
            notice(&newsletters, &settings).unwrap().actions[0],
            RuleAction::MoveToStream {
                stream: Stream::Newsletters
            }
        );
    }

    #[test]
    fn mail_archived_unread_is_archived_on_arrival() {
        let alerts = SenderStats {
            sender: "jobs@alerts.example".into(),
            received: 48,
            opened: 6,
            archived_unread: 40,
            ..builds()
        };
        let noticed = notice(&alerts, &NoticeSettings::default()).unwrap();
        assert_eq!(noticed.kind, NoticeKind::ArchiveOnArrival);
        assert_eq!(noticed.actions[0], RuleAction::Archive);
    }

    #[test]
    fn what_matters_is_never_noticed() {
        let settings = NoticeSettings::default();
        for (why, stats) in [
            (
                "few",
                SenderStats {
                    received: 5,
                    ..builds()
                },
            ),
            (
                "one burst",
                SenderStats {
                    days: 2,
                    ..builds()
                },
            ),
            (
                // 8 of 31 is just over a quarter: never rounded down to 25%.
                "opened over a quarter",
                SenderStats {
                    received: 31,
                    opened: 8,
                    ..builds()
                },
            ),
            (
                "replied",
                SenderStats {
                    replied: true,
                    ..builds()
                },
            ),
            (
                "suspicious",
                SenderStats {
                    suspicious: true,
                    ..builds()
                },
            ),
            (
                "asks",
                SenderStats {
                    asked: true,
                    ..builds()
                },
            ),
            (
                "a bill",
                SenderStats {
                    owed: true,
                    ..builds()
                },
            ),
            (
                "a person",
                SenderStats {
                    machine: false,
                    ..builds()
                },
            ),
            (
                "written to",
                SenderStats {
                    correspondent: true,
                    ..builds()
                },
            ),
            (
                "read",
                SenderStats {
                    opened: 20,
                    ..builds()
                },
            ),
        ] {
            assert_eq!(notice(&stats, &settings), None, "{why}");
        }
    }

    #[test]
    fn cadence_reads_the_median_gap() {
        assert_eq!(cadence(&every(24, 10)), Some(Cadence::Daily));
        assert_eq!(cadence(&every(24 * 7, 6)), Some(Cadence::Weekly));
        assert_eq!(cadence(&every(24 * 30, 5)), Some(Cadence::Monthly));
        assert_eq!(cadence(&every(24 * 3, 6)), None);
        assert_eq!(cadence(&every(24, 2)), None);
    }

    #[test]
    fn thresholds_come_from_the_settings() {
        let strict = NoticeSettings {
            min_messages: 50,
            ..NoticeSettings::default()
        };
        assert_eq!(notice(&builds(), &strict), None);
    }

    // --- Kinds within a sender's mail (campbooks-cloud#356) ---------------

    const SHOP: &str = "orders@shop.example";

    /// The day mail of a test is counted on: the window's end.
    fn now() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-09-30T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    /// One conversation `days_ago` with `subject`, its template worked out
    /// as stored, opened or not.
    fn mail(subject: &str, days_ago: i64, opened: bool) -> Mail {
        Mail {
            subject: subject.to_owned(),
            template: campbooks_ai::decide::signals::email::subject_template(subject),
            sent_at: now() - TimeDelta::days(days_ago) - TimeDelta::hours(1),
            opened,
            archived_unread: false,
            replied: false,
            suspicious: false,
            owed: false,
            asked: false,
            stream: Stream::Updates,
        }
    }

    /// A shop sending `mail`, newest first as the pass reads it.
    fn shop(mut mail: Vec<Mail>) -> Sender {
        mail.sort_by_key(|m| std::cmp::Reverse(m.sent_at));
        Sender {
            address: SHOP.to_owned(),
            machine: true,
            correspondent: false,
            mail,
        }
    }

    /// `n` weekly deals, one a day from `from_day` back, none opened.
    fn deals(n: i64, from_day: i64) -> Vec<Mail> {
        (0..n)
            .map(|i| {
                mail(
                    &format!("Deals of the week {}", 40 - i),
                    from_day + i,
                    false,
                )
            })
            .collect()
    }

    /// `n` shipping notices, every other day, all read.
    fn shipping(n: i64) -> Vec<Mail> {
        (0..n)
            .map(|i| {
                mail(
                    &format!("Your order {} has shipped", 5100 + i),
                    2 * i + 1,
                    true,
                )
            })
            .collect()
    }

    fn template_of(noticed: &Noticed) -> Option<&str> {
        match &noticed.pattern {
            Pattern::Template(template) => Some(template),
            _ => None,
        }
    }

    #[test]
    fn a_sender_of_one_kind_gets_one_rule_for_all_of_it() {
        let settings = NoticeSettings::default();
        let builds = shop(
            (0..20)
                .map(|i| mail(&format!("Build {} passed", 900 + i), i, false))
                .collect(),
        );
        let reading = read(&builds, &[], &settings, now());
        assert!(!reading.to_group);
        assert_eq!(reading.notices.len(), 1, "{reading:?}");
        let noticed = &reading.notices[0];
        assert_eq!(noticed.pattern, Pattern::Sender);
        assert_eq!(noticed.example, None);
        assert_eq!(
            noticed.conditions,
            [RuleCondition::SenderAddress(SHOP.into())]
        );
        assert_eq!(noticed.evidence.received, 20);
        assert!(uniform(&builds, &[], &settings, now()));
    }

    #[test]
    fn a_shop_whose_deals_are_ignored_and_shipping_read_is_offered_the_deals_only() {
        let settings = NoticeSettings::default();
        let mut all = deals(12, 0);
        all.extend(shipping(6));
        let sender = shop(all);
        let reading = read(&sender, &[], &settings, now());
        assert_eq!(reading.notices.len(), 1, "{reading:?}");
        let deals = &reading.notices[0];
        assert_eq!(deals.kind, NoticeKind::KeepOffHome);
        assert_eq!(template_of(deals), Some("deals of the week #"));
        assert_eq!(
            deals.conditions,
            [
                RuleCondition::SenderAddress(SHOP.into()),
                RuleCondition::SubjectTemplate("deals of the week #".into()),
            ]
        );
        // The newest of them, as a real example; the kind's own counts.
        assert_eq!(deals.example.as_deref(), Some("Deals of the week 40"));
        assert_eq!(deals.evidence.received, 12);
        assert_eq!(deals.evidence.opened, 0);
        assert!(!uniform(&sender, &[], &settings, now()));
        // Nothing left over: no reason to ask the model.
        assert!(!reading.to_group);
    }

    #[test]
    fn a_bill_blocks_only_its_own_kind() {
        let settings = NoticeSettings::default();
        let mut all = deals(10, 0);
        let mut invoices: Vec<Mail> = (0..5)
            .map(|i| mail(&format!("Your invoice for month {}", i + 1), 3 * i, false))
            .collect();
        invoices[2].owed = true;
        all.extend(invoices);
        let reading = read(&shop(all), &[], &settings, now());
        assert_eq!(reading.notices.len(), 1, "{reading:?}");
        assert_eq!(
            template_of(&reading.notices[0]),
            Some("deals of the week #")
        );
    }

    #[test]
    fn suspicious_mail_anywhere_blocks_the_whole_sender() {
        let settings = NoticeSettings::default();
        let mut all = deals(12, 0);
        all.extend(shipping(6));
        all[17].suspicious = true;
        assert_eq!(read(&shop(all), &[], &settings, now()), Reading::default());
        // People and those the inbox writes to too, whatever their kinds.
        let mut person = shop(deals(12, 0));
        person.machine = false;
        assert!(read(&person, &[], &settings, now()).notices.is_empty());
        let mut written = shop(deals(12, 0));
        written.correspondent = true;
        assert!(read(&written, &[], &settings, now()).notices.is_empty());
    }

    #[test]
    fn mixed_mail_with_no_kind_to_offer_gets_nothing() {
        let settings = NoticeSettings::default();
        // Half the deals are read; one shipping notice was answered.
        let mut all: Vec<Mail> = deals(8, 0)
            .into_iter()
            .enumerate()
            .map(|(i, m)| Mail {
                opened: i % 2 == 0,
                ..m
            })
            .collect();
        let mut parcels = shipping(6);
        parcels[0].replied = true;
        all.extend(parcels);
        let sender = shop(all);
        let reading = read(&sender, &[], &settings, now());
        // Never a rule for the whole sender instead.
        assert!(reading.notices.is_empty(), "{reading:?}");
        assert!(!uniform(&sender, &[], &settings, now()));
    }

    /// Ten promotions, each with a subject of its own, none opened.
    fn sales() -> Vec<Mail> {
        [
            "boots", "scarves", "coats", "gloves", "hats", "jackets", "socks", "shirts", "belts",
            "bags",
        ]
        .iter()
        .zip(0..)
        .map(|(item, i)| mail(&format!("Our {item} sale starts now"), i, false))
        .collect()
    }

    #[test]
    fn mail_left_over_is_grouped_by_the_models_words_and_checked_against_the_mail() {
        let settings = NoticeSettings::default();
        let mut all = sales();
        all.extend(shipping(6));
        let sender = shop(all);
        // No template splits the promotions: the model may group them.
        let reading = read(&sender, &[], &settings, now());
        assert!(reading.to_group);
        assert!(reading.notices.is_empty(), "{reading:?}");

        // Its words: one fits the promotions, one fits too little mail, one
        // fits the shipping notices the member reads.
        let phrases = ["sale starts now", "boots", "has shipped"].map(String::from);
        let reading = read(&sender, &phrases, &settings, now());
        assert_eq!(reading.notices.len(), 1, "{reading:?}");
        let sales = &reading.notices[0];
        assert_eq!(sales.pattern, Pattern::Phrase("sale starts now".into()));
        assert_eq!(
            sales.conditions,
            [
                RuleCondition::SenderAddress(SHOP.into()),
                RuleCondition::SubjectContains("sale starts now".into()),
            ]
        );
        assert_eq!(sales.evidence.received, 10);
        assert_eq!(sales.example.as_deref(), Some("Our boots sale starts now"));
    }

    #[test]
    fn the_models_words_never_repeat_a_kind_already_offered() {
        let settings = NoticeSettings::default();
        let mut all = sales();
        all.extend(deals(6, 0));
        all.extend(shipping(6));
        let sender = shop(all);
        let phrases = ["deals of the week", "sale", "sale starts"].map(String::from);
        let reading = read(&sender, &phrases, &settings, now());
        let patterns: Vec<&Pattern> = reading.notices.iter().map(|n| &n.pattern).collect();
        // The deals' template is a kind already; "sale starts" is the same
        // mail as "sale".
        assert_eq!(
            patterns,
            [
                &Pattern::Phrase("sale".into()),
                &Pattern::Template("deals of the week #".into()),
            ]
        );
    }

    /// A newsletter: `promos` promotions none opened, `articles` stories
    /// all opened, every subject its own.
    fn newsletter(promos: usize, articles: usize) -> Vec<Mail> {
        let topics = [
            "spring", "summer", "autumn", "winter", "travel", "cooking", "gardens", "music",
            "cinema", "design", "science", "history",
        ];
        let mut mail: Vec<Mail> = topics
            .iter()
            .take(promos)
            .zip(0..)
            .map(|(topic, i)| mail(&format!("Exclusive offer on {topic} picks"), 2 * i, false))
            .collect();
        mail.extend(
            topics
                .iter()
                .rev()
                .take(articles)
                .zip(0..)
                .map(|(topic, i)| mail(&format!("This week in {topic}"), 2 * i + 1, true)),
        );
        mail
    }

    #[test]
    fn a_mixed_sender_whose_subjects_all_differ_is_left_to_the_models_words() {
        let settings = NoticeSettings::default();
        let sender = shop(newsletter(8, 6));
        // Opened 6 of 14: nothing for the whole sender, no template to split
        // it by, so the model is asked.
        let reading = read(&sender, &[], &settings, now());
        assert!(reading.to_group);
        assert!(reading.notices.is_empty(), "{reading:?}");
        let phrases = ["exclusive offer".to_owned(), "this week in".to_owned()];
        let reading = read(&sender, &phrases, &settings, now());
        assert_eq!(reading.notices.len(), 1, "{reading:?}");
        assert_eq!(
            reading.notices[0].pattern,
            Pattern::Phrase("exclusive offer".into())
        );
        assert!(!uniform(&sender, &phrases, &settings, now()));
    }

    #[test]
    fn mail_all_of_one_kind_is_never_sent_to_the_model() {
        let settings = NoticeSettings::default();
        // Every subject its own, none opened: one rule for all of it.
        let sender = shop(newsletter(12, 0));
        let reading = read(&sender, &[], &settings, now());
        assert!(!reading.to_group);
        assert_eq!(reading.notices.len(), 1, "{reading:?}");
        assert_eq!(reading.notices[0].pattern, Pattern::Sender);
        // Mail all read neither: nothing in it to offer.
        let read_all: Vec<Mail> = newsletter(12, 0)
            .into_iter()
            .map(|m| Mail { opened: true, ..m })
            .collect();
        assert!(!read(&shop(read_all), &[], &settings, now()).to_group);
    }

    #[test]
    fn a_kind_the_model_found_and_the_member_reads_keeps_the_whole_sender_unoffered() {
        let settings = NoticeSettings::default();
        // 12 promotions unopened and 4 stories read: a quarter opened, so
        // the whole sender alone would be kept off Home.
        let sender = shop(newsletter(12, 4));
        let reading = read(&sender, &[], &settings, now());
        assert_eq!(reading.notices[0].pattern, Pattern::Sender);
        assert!(reading.to_group);
        // The model's words tell the stories apart: only the promotions.
        let phrases = ["exclusive offer".to_owned(), "this week in".to_owned()];
        let reading = read(&sender, &phrases, &settings, now());
        let patterns: Vec<&Pattern> = reading.notices.iter().map(|n| &n.pattern).collect();
        assert_eq!(patterns, [&Pattern::Phrase("exclusive offer".into())]);
        assert!(uniform(&sender, &[], &settings, now()));
        assert!(!uniform(&sender, &phrases, &settings, now()));
    }

    #[test]
    fn patterns_are_stored_and_read_back() {
        for pattern in [
            Pattern::Sender,
            Pattern::Template("your order # has shipped".into()),
            Pattern::Phrase("sale starts now".into()),
        ] {
            assert_eq!(Pattern::of_key(&pattern.key()), pattern);
        }
        assert_eq!(Pattern::Sender.key(), "");
        assert_eq!(Pattern::of_key("p:sale").key(), "p:sale");
    }
}
