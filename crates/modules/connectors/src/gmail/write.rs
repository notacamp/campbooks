//! Making a stream's pending outbound changes at Gmail: merge them, drop
//! those a later Gmail change overtook, write the rest, and say how each
//! ended. Talks to Gmail only; the run records the [`Push`] it returns.
//!
//! The rules (design of the `gmail-writeback` change):
//! - per message and label the latest recorded change wins; earlier ones
//!   end `superseded` (`replaced`);
//! - a Gmail history record changing the same label after the winning
//!   change's stamp, outside the window of one of our own writes, is a later
//!   writer: the change ends `superseded` (`changed_at_provider`) and the
//!   message is re-emitted;
//! - more than [`BATCH_FROM`]` - 1` messages with the same change go in one
//!   `batchModify`, the rest are modified one by one;
//! - the trash is made with `messages.trash` and taken back with
//!   `messages.untrash` (Gmail puts back the labels the message had), only
//!   for a message the index has in the trash (or does not carry); reporting
//!   spam and "not spam" are label changes (`outbound::map`), so a report on
//!   mail that is not in the trash is batched like any other.

use std::collections::{BTreeMap, HashMap};

use futures_util::{StreamExt as _, stream::FuturesOrdered};

use super::FETCH_CONCURRENCY;
use crate::{
    domain::{
        outbound::{Backlog, LabelChange, apply_labels},
        push::{Fate, Fates, Plan, Push, Results, Wants, retry_or_give_up},
    },
    google::Gmail,
    provider::ProviderError,
};

/// Groups of at least this many messages with the same change use
/// `batchModify` (50 units) rather than `modify` (5 each).
pub const BATCH_FROM: usize = 11;

/// Most messages in one `batchModify`.
pub const BATCH_MAX: usize = 1_000;

/// History pages read for the later-writer check.
pub const HISTORY_PAGES: usize = 4;

/// Gmail quota units of `getProfile`.
const PROFILE_UNITS: u32 = 1;
/// Of a `history.list` page.
const HISTORY_UNITS: u32 = 2;
/// Of `batchModify`.
const BATCH_UNITS: u32 = 50;
/// Of `modify`, `trash` and `untrash` each.
const CALL_UNITS: u32 = 5;

/// Gmail quota units pushing `backlog` may spend: the profile before and
/// after, the history pages, and the writes it plans (a batch per chunk;
/// per message its untrash, modify and trash, as each is needed). Later
/// writers only take writes away.
pub fn units(backlog: &Backlog) -> u32 {
    let writes = Writes::plan(&Plan::merge(backlog).groups(), &backlog.labels);
    let pages = u32::try_from(HISTORY_PAGES).unwrap_or(u32::MAX);
    let batches = u32::try_from(writes.batches.len()).unwrap_or(u32::MAX);
    let calls = writes
        .singles
        .iter()
        .map(Single::calls)
        .fold(0_u32, u32::saturating_add);
    (2 * PROFILE_UNITS)
        .saturating_add(pages.saturating_mul(HISTORY_UNITS))
        .saturating_add(batches.saturating_mul(BATCH_UNITS))
        .saturating_add(calls.saturating_mul(CALL_UNITS))
}

/// Push `backlog` through `gmail`.
pub async fn push(gmail: &Gmail<'_>, backlog: &Backlog) -> Push {
    let mut push = Push::default();
    let mut plan = Plan::merge(backlog);
    // Later writers at Gmail.
    let before = match overtaken(gmail, backlog, &plan.wants, &mut plan.fates).await {
        Ok(before) => before,
        Err(trouble) => {
            for change in &backlog.pending {
                let end = retry_or_give_up(
                    change.id,
                    change.attempts,
                    &mut push.outcome.reemit,
                    &change.provider_message_id,
                );
                push.outcome.ends.push(end);
            }
            push.trouble = Some(trouble);
            return push;
        }
    };
    plan.atomic();
    let mut done = Results::default();
    let writes = Writes::plan(&plan.groups(), &backlog.labels);
    push.trouble = write(gmail, writes, &mut done).await;
    // After writing: the window our writes fall in.
    if !done.written.is_empty() {
        let after = gmail
            .profile()
            .await
            .ok()
            .and_then(|p| p.history_id.parse::<i64>().ok());
        push.outcome.window = before.map(|from| (from, after));
    }
    plan.ends(backlog, &done, &mut push.outcome);
    // The index holds what we set: Gmail's record of the write then
    // compares equal and nothing is emitted for it.
    for message in &done.written {
        let labels = apply_labels(&backlog.labels[message], &[plan.change_of(message)]);
        push.outcome.index.push((message.clone(), labels));
    }
    push.outcome.index.sort();
    push.outcome.reemit.sort();
    push.outcome.reemit.dedup();
    push
}

/// One message's write: its labels, and whether it goes into the trash
/// (`Some(true)`) or out of it (`Some(false)`) first.
#[derive(Debug, PartialEq, Eq)]
struct Single {
    message: String,
    add: Vec<String>,
    remove: Vec<String>,
    trash: Option<bool>,
}

impl Single {
    /// The Gmail calls it makes ([`write_one`]).
    fn calls(&self) -> u32 {
        u32::from(self.trash.is_some()) + u32::from(!self.add.is_empty() || !self.remove.is_empty())
    }
}

/// What a push writes: batches (one chunk each), messages one by one, and
/// messages with nothing left to write.
#[derive(Debug, Default, PartialEq, Eq)]
struct Writes {
    batches: Vec<(LabelChange, Vec<String>)>,
    singles: Vec<Single>,
    nothing: Vec<String>,
}

impl Writes {
    /// Plan `groups` against the index's `labels`. Into the trash is
    /// `messages.trash` per message. Out of it is `messages.untrash` per
    /// message, but only for a message the index has in the trash (or does
    /// not carry): for the rest, taking TRASH off is nothing, and the
    /// change's other labels are written like any group's.
    fn plan(
        groups: &BTreeMap<LabelChange, Vec<String>>,
        labels: &HashMap<String, Vec<String>>,
    ) -> Self {
        let mut writes = Self::default();
        for ((add, remove), messages) in groups {
            let (into, out_of) = (has(add, "TRASH"), has(remove, "TRASH"));
            if !into && !out_of {
                writes.group(add, remove, messages);
                continue;
            }
            let without = |list: &[String]| -> Vec<String> {
                list.iter().filter(|l| *l != "TRASH").cloned().collect()
            };
            let (add, remove) = (without(add), without(remove));
            let mut plain = Vec::new();
            for message in messages {
                let trashed = labels.get(message).is_none_or(|l| has(l, "TRASH"));
                if into || trashed {
                    writes.singles.push(Single {
                        message: message.clone(),
                        add: add.clone(),
                        remove: remove.clone(),
                        trash: Some(into),
                    });
                } else {
                    plain.push(message.clone());
                }
            }
            writes.group(&add, &remove, &plain);
        }
        writes
    }

    /// `messages` all getting (`add`, `remove`): nothing, one by one, or in
    /// batches.
    fn group(&mut self, add: &[String], remove: &[String], messages: &[String]) {
        if add.is_empty() && remove.is_empty() {
            self.nothing.extend(messages.iter().cloned());
        } else if messages.len() < BATCH_FROM {
            self.singles.extend(messages.iter().map(|m| Single {
                message: m.clone(),
                add: add.to_vec(),
                remove: remove.to_vec(),
                trash: None,
            }));
        } else {
            for chunk in messages.chunks(BATCH_MAX) {
                self.batches
                    .push(((add.to_vec(), remove.to_vec()), chunk.to_vec()));
            }
        }
    }
}

/// Write `writes`: each batch (a batch Gmail refused is written one message
/// at a time), then the messages one by one. Returns the trouble that
/// stopped it.
async fn write(gmail: &Gmail<'_>, writes: Writes, done: &mut Results) -> Option<ProviderError> {
    let Writes {
        batches,
        mut singles,
        nothing,
    } = writes;
    done.written.extend(nothing);
    for ((add, remove), chunk) in &batches {
        match gmail.batch_modify(chunk, add, remove).await {
            Ok(()) => done.written.extend(chunk.iter().cloned()),
            // Refused as a whole: find the message that refuses by asking
            // one at a time.
            Err(ProviderError::NotFound | ProviderError::Unexpected(_)) => {
                singles.extend(chunk.iter().map(|m| Single {
                    message: m.clone(),
                    add: add.clone(),
                    remove: remove.clone(),
                    trash: None,
                }));
            }
            Err(trouble) => return Some(trouble),
        }
    }
    modify_each(gmail, &singles, done).await
}

/// Whether `list` holds `label`.
fn has(list: &[String], label: &str) -> bool {
    list.iter().any(|l| l == label)
}

/// Make one message's write: out of the trash first, then its labels, then
/// into the trash.
async fn write_one(gmail: &Gmail<'_>, single: &Single) -> Result<(), ProviderError> {
    if single.trash == Some(false) {
        gmail.untrash(&single.message).await?;
    }
    if !single.add.is_empty() || !single.remove.is_empty() {
        gmail
            .modify(&single.message, &single.add, &single.remove)
            .await?;
    }
    if single.trash == Some(true) {
        gmail.trash(&single.message).await?;
    }
    Ok(())
}

/// Read Gmail's history from the oldest stamp and mark the (message, label)
/// pairs a later Gmail change overtook, and the messages Gmail deleted.
/// Returns Gmail's history id before any write (the start of our write
/// window), when it could be read.
async fn overtaken(
    gmail: &Gmail<'_>,
    backlog: &Backlog,
    wants: &Wants,
    fates: &mut Fates,
) -> Result<Option<i64>, ProviderError> {
    let oldest = wants
        .values()
        .flat_map(BTreeMap::values)
        .filter_map(|w| w.stamp)
        .min();
    let Some(oldest) = oldest else {
        return profile_id(gmail).await;
    };
    let start = oldest.to_string();
    let mut token: Option<String> = None;
    let mut latest = None;
    for _ in 0..HISTORY_PAGES {
        let page = match gmail.history(&start, token.as_deref(), 500).await {
            Ok(page) => page,
            // Gmail no longer has that far back: arrival order.
            Err(ProviderError::NotFound) => return profile_id(gmail).await,
            Err(other) => return Err(other),
        };
        latest = page.history_id.parse::<i64>().ok();
        for record in &page.history {
            let Ok(at) = record.id.parse::<i64>() else {
                continue;
            };
            for deleted in &record.messages_deleted {
                for ((message, _), fate) in
                    fates.range_mut((deleted.message.id.clone(), String::new())..)
                {
                    if *message != deleted.message.id {
                        break;
                    }
                    *fate = Fate::Gone;
                }
            }
            for changed in record.labels_added.iter().chain(&record.labels_removed) {
                let message = &changed.message.id;
                let Some(labels) = wants.get(message) else {
                    continue;
                };
                for label in &changed.label_ids {
                    let Some(want) = labels.get(label) else {
                        continue;
                    };
                    let later = want.stamp.is_some_and(|stamp| at > stamp);
                    let ours = backlog
                        .own
                        .get(message)
                        .is_some_and(|own| own.iter().any(|w| w.covers(label, at)));
                    let key = (message.clone(), label.clone());
                    if later && !ours && fates[&key] == Fate::Write {
                        fates.insert(key, Fate::Overtaken);
                    }
                }
            }
        }
        token = page.next_page_token;
        if token.is_none() {
            return Ok(latest);
        }
    }
    // More history than we read: what was read stands, the rest is
    // arrival order. Our window starts no earlier than now.
    Ok(profile_id(gmail).await?.or(latest))
}

/// Gmail's current history id.
async fn profile_id(gmail: &Gmail<'_>) -> Result<Option<i64>, ProviderError> {
    Ok(gmail.profile().await?.history_id.parse::<i64>().ok())
}

/// Write each of `singles` ([`write_one`]), [`FETCH_CONCURRENCY`] at a
/// time, starting none after trouble. Returns the trouble.
async fn modify_each(
    gmail: &Gmail<'_>,
    singles: &[Single],
    done: &mut Results,
) -> Option<ProviderError> {
    let mut trouble = None;
    let mut next = 0;
    let mut in_flight = FuturesOrdered::new();
    loop {
        while trouble.is_none() && in_flight.len() < FETCH_CONCURRENCY && next < singles.len() {
            let single = &singles[next];
            next += 1;
            in_flight.push_back(async move { (&single.message, write_one(gmail, single).await) });
        }
        let Some((message, result)) = in_flight.next().await else {
            break;
        };
        match result {
            Ok(()) => {
                done.written.insert(message.clone());
            }
            Err(ProviderError::NotFound) => {
                done.gone.insert(message.clone());
            }
            Err(ProviderError::Unexpected(_)) => {
                tracing::info!("Gmail refused a change to one message");
                done.refused.insert(message.clone());
            }
            Err(other) => {
                trouble.get_or_insert(other);
            }
        }
    }
    trouble
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(list: &[&str]) -> Vec<String> {
        list.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn only_mail_in_the_trash_is_untrashed() {
        // Reports: SPAM on, INBOX and TRASH off. Twelve messages not in the
        // trash and one in it.
        let report = (strings(&["SPAM"]), strings(&["INBOX", "TRASH"]));
        let mut messages: Vec<String> = (0..12).map(|i| format!("m{i:02}")).collect();
        messages.push("trashed".to_owned());
        let mut labels: HashMap<String, Vec<String>> = messages
            .iter()
            .map(|m| (m.clone(), strings(&["INBOX"])))
            .collect();
        labels.insert("trashed".to_owned(), strings(&["TRASH"]));
        let groups = BTreeMap::from([(report, messages.clone())]);
        let writes = Writes::plan(&groups, &labels);
        // The twelve go in one batch, with no untrash.
        assert_eq!(
            writes.batches,
            vec![(
                (strings(&["SPAM"]), strings(&["INBOX"])),
                messages[..12].to_vec()
            )]
        );
        // The trashed one is untrashed, then reported.
        assert_eq!(
            writes.singles,
            vec![Single {
                message: "trashed".to_owned(),
                add: strings(&["SPAM"]),
                remove: strings(&["INBOX"]),
                trash: Some(false),
            }]
        );
        assert_eq!(writes.singles[0].calls(), 2);

        // An untrash of a message the index has out of the trash is nothing.
        let untrash = (Vec::new(), strings(&["TRASH"]));
        let groups = BTreeMap::from([(untrash, strings(&["m00"]))]);
        let writes = Writes::plan(&groups, &labels);
        assert_eq!(writes.nothing, strings(&["m00"]));
        assert!(writes.singles.is_empty() && writes.batches.is_empty());
    }
}
