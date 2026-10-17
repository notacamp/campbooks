//! What Scout notices about one sender's mail, from facts and counts only:
//! how often and how regularly it comes (addresses and dates), what it is
//! (the stream the decide pass sorted it to, whether a payment is owed,
//! whether it looked suspicious) and what the inbox does with it (opened,
//! archived or deleted unread, replied). No subject, body or attachment is
//! read, and no phrase list: what the mail says is the model's, and the
//! stream it chose is all this uses of it (ADR 0018).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    config::NoticeSettings,
    domain::rules::{RuleAction, RuleCondition, SurfaceWhen},
    model::{NoticeKind, Stream},
};

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

/// A rule Scout would offer for a sender.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Noticed {
    /// What it noticed.
    pub kind: NoticeKind,
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

/// The rule to offer for `stats`, if any, under `settings`. Never for a
/// person the inbox writes to, for mail that looked suspicious, asked
/// something of the member or asks for a payment, nor for mail the member
/// answers or reads: those matter.
pub fn notice(stats: &SenderStats, settings: &NoticeSettings) -> Option<Noticed> {
    if stats.received < settings.min_messages || stats.days < settings.min_days {
        return None;
    }
    if stats.replied
        || stats.suspicious
        || stats.asked
        || stats.owed
        || stats.correspondent
        || !stats.machine
    {
        return None;
    }
    let percent = |n: u32| u64::from(n) * 100 / u64::from(stats.received.max(1));
    let (kind, actions) =
        if percent(stats.archived_unread) >= u64::from(settings.archived_unread_percent_min) {
            (
                NoticeKind::ArchiveOnArrival,
                vec![
                    RuleAction::Archive,
                    RuleAction::Surface {
                        value: SurfaceWhen::Never,
                    },
                ],
            )
        } else if percent(stats.opened) <= u64::from(settings.opened_percent_max) {
            // Filed where the decide pass sorts most of it; mail it keeps in
            // Primary goes to Updates.
            let stream = if stats.stream == Stream::Primary {
                Stream::Updates
            } else {
                stats.stream
            };
            (
                NoticeKind::KeepOffHome,
                vec![
                    RuleAction::MoveToStream { stream },
                    RuleAction::Surface {
                        value: SurfaceWhen::Never,
                    },
                ],
            )
        } else {
            return None;
        };
    Some(Noticed {
        kind,
        conditions: vec![RuleCondition::SenderAddress(stats.sender.clone())],
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
}
