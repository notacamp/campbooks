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
//!   `batchModify`, the rest are modified one by one.

use std::collections::{BTreeMap, HashMap, HashSet};

use futures_util::{StreamExt as _, stream::FuturesOrdered};
use uuid::Uuid;

use super::FETCH_CONCURRENCY;
use crate::{
    domain::outbound::{Backlog, End, LabelChange, MAX_ATTEMPTS, Outcome, State, apply_labels},
    google::{Gmail, GoogleError},
};

/// Groups of at least this many messages with the same change use
/// `batchModify` (50 units) rather than `modify` (5 each).
pub const BATCH_FROM: usize = 11;

/// Most messages in one `batchModify`.
pub const BATCH_MAX: usize = 1_000;

/// History pages read for the later-writer check.
pub const HISTORY_PAGES: usize = 4;

/// Gmail quota units a push of `messages` messages may spend: a profile
/// call, the history pages and one modify per message.
pub fn units(messages: usize) -> u32 {
    let messages = u32::try_from(messages).unwrap_or(u32::MAX);
    1 + 2 * u32::try_from(HISTORY_PAGES).unwrap_or(4) + messages.saturating_mul(5)
}

/// What a push did, and the provider trouble that stopped it, if any.
#[derive(Debug, Default)]
pub struct Push {
    /// To record.
    pub outcome: Outcome,
    /// Gmail failed in a way the run's backoff handles.
    pub trouble: Option<GoogleError>,
}

/// One label's wanted state on one message: the change that won it.
#[derive(Debug, Clone)]
struct Want {
    present: bool,
    row: Uuid,
    stamp: Option<i64>,
}

/// How a won (message, label) ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Fate {
    Write,
    Overtaken,
    Gone,
}

/// Wanted label states, per message then label.
type Wants = BTreeMap<String, BTreeMap<String, Want>>;

/// How each wanted (message, label) ends.
type Fates = BTreeMap<(String, String), Fate>;

/// The merged changes of a backlog and how each is to end.
struct Plan {
    wants: Wants,
    fates: Fates,
    /// The (message, label) pairs each change won.
    won: HashMap<Uuid, Vec<(String, String)>>,
    /// Changes that would bring a message in the trash or spam back into
    /// the inbox or a category: skipped.
    in_trash: HashSet<Uuid>,
}

/// What the writes did, per message.
#[derive(Debug, Default)]
struct Results {
    written: HashSet<String>,
    refused: HashSet<String>,
    gone: HashSet<String>,
}

impl Plan {
    /// Merge: per message and label, the latest change wins. Messages the
    /// index does not carry are gone.
    fn merge(backlog: &Backlog) -> Self {
        let mut wants = Wants::new();
        let mut in_trash = HashSet::new();
        for change in &backlog.pending {
            let trashed = backlog
                .labels
                .get(&change.provider_message_id)
                .is_some_and(|labels| labels.iter().any(|l| l == "TRASH" || l == "SPAM"));
            let restores = change
                .add
                .iter()
                .any(|l| l == "INBOX" || l.starts_with("CATEGORY_"));
            if trashed && restores {
                in_trash.insert(change.id);
                continue;
            }
            let labels = wants.entry(change.provider_message_id.clone()).or_default();
            let adds = change.add.iter().map(|l| (l, true));
            for (label, present) in adds.chain(change.remove.iter().map(|l| (l, false))) {
                let want = Want {
                    present,
                    row: change.id,
                    stamp: change.stamp,
                };
                labels.insert(label.clone(), want);
            }
        }
        let mut fates = Fates::new();
        for (message, labels) in &wants {
            let fate = if backlog.labels.contains_key(message) {
                Fate::Write
            } else {
                Fate::Gone
            };
            for label in labels.keys() {
                fates.insert((message.clone(), label.clone()), fate);
            }
        }
        let mut won: HashMap<Uuid, Vec<(String, String)>> = HashMap::new();
        for (message, labels) in &wants {
            for (label, want) in labels {
                won.entry(want.row)
                    .or_default()
                    .push((message.clone(), label.clone()));
            }
        }
        Self {
            wants,
            fates,
            won,
            in_trash,
        }
    }

    /// A change is made whole or not at all: when any label it won was
    /// changed at Gmail later, none of it is written (a move between
    /// categories must never leave two).
    fn atomic(&mut self) {
        for pairs in self.won.values() {
            if pairs.iter().any(|p| self.fates[p] == Fate::Overtaken) {
                for pair in pairs {
                    if self.fates[pair] == Fate::Write {
                        self.fates.insert(pair.clone(), Fate::Overtaken);
                    }
                }
            }
        }
    }

    /// What is still to be written on `message`.
    fn change_of(&self, message: &str) -> LabelChange {
        let (mut add, mut remove) = (Vec::new(), Vec::new());
        for (label, want) in self.wants.get(message).into_iter().flatten() {
            if self.fates[&(message.to_owned(), label.clone())] == Fate::Write {
                if want.present {
                    add.push(label.clone());
                } else {
                    remove.push(label.clone());
                }
            }
        }
        (add, remove)
    }

    /// The messages to write, grouped by identical change.
    fn groups(&self) -> BTreeMap<LabelChange, Vec<String>> {
        let mut groups: BTreeMap<LabelChange, Vec<String>> = BTreeMap::new();
        for message in self.wants.keys() {
            let change = self.change_of(message);
            if !change.0.is_empty() || !change.1.is_empty() {
                groups.entry(change).or_default().push(message.clone());
            }
        }
        groups
    }

    /// How each change of `backlog` ended, into `outcome`.
    fn ends(&self, backlog: &Backlog, done: &Results, outcome: &mut Outcome) {
        for change in &backlog.pending {
            let (id, message) = (change.id, &change.provider_message_id);
            if self.in_trash.contains(&id) {
                // Gmail has it in the trash or spam: Campbooks is told so.
                outcome.reemit.push(message.clone());
                outcome.ends.push(End {
                    id,
                    state: State::Skipped,
                    reason: Some("in_trash"),
                });
                continue;
            }
            let pairs = self.won.get(&id).map(Vec::as_slice).unwrap_or_default();
            let fate_is = |fate| move |p: &(String, String)| self.fates[p] == fate;
            let ended = |state, reason| End { id, state, reason };
            let end = if pairs.is_empty() {
                ended(State::Superseded, Some("replaced"))
            } else if done.written.contains(message) && pairs.iter().any(fate_is(Fate::Write)) {
                ended(State::Applied, None)
            } else if done.refused.contains(message) {
                outcome.reemit.push(message.clone());
                ended(State::Failed, Some("refused"))
            } else if done.gone.contains(message) || pairs.iter().all(fate_is(Fate::Gone)) {
                ended(State::Skipped, Some("message_gone"))
            } else if pairs.iter().any(fate_is(Fate::Overtaken)) {
                outcome.reemit.push(message.clone());
                ended(State::Superseded, Some("changed_at_provider"))
            } else {
                // Not written: the push stopped on trouble.
                retry_or_give_up(id, change.attempts, &mut outcome.reemit, message)
            };
            outcome.ends.push(end);
        }
    }
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
    push.trouble = write(gmail, &plan.groups(), &mut done).await;
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

/// Write `groups`: a batch per chunk of a large group, one call per message
/// otherwise (and for a batch Gmail refused). Returns the trouble that
/// stopped it.
async fn write(
    gmail: &Gmail<'_>,
    groups: &BTreeMap<LabelChange, Vec<String>>,
    done: &mut Results,
) -> Option<GoogleError> {
    let mut singles: Vec<(String, Vec<String>, Vec<String>)> = Vec::new();
    for ((add, remove), messages) in groups {
        let one_by_one = |list: &[String]| {
            list.iter()
                .map(|m| (m.clone(), add.clone(), remove.clone()))
                .collect::<Vec<_>>()
        };
        if messages.len() < BATCH_FROM {
            singles.extend(one_by_one(messages));
            continue;
        }
        for chunk in messages.chunks(BATCH_MAX) {
            match gmail.batch_modify(chunk, add, remove).await {
                Ok(()) => done.written.extend(chunk.iter().cloned()),
                // Refused as a whole: find the message that refuses by
                // asking one at a time.
                Err(GoogleError::NotFound | GoogleError::Unexpected(_)) => {
                    singles.extend(one_by_one(chunk));
                }
                Err(trouble) => return Some(trouble),
            }
        }
    }
    modify_each(gmail, &singles, done).await
}

/// A change the push did not get to because of trouble: pending again, or
/// failed once it has met trouble [`MAX_ATTEMPTS`] times.
fn retry_or_give_up(id: Uuid, attempts: i32, reemit: &mut Vec<String>, message: &str) -> End {
    if attempts + 1 >= MAX_ATTEMPTS {
        reemit.push(message.to_owned());
        End {
            id,
            state: State::Failed,
            reason: Some("provider_errors"),
        }
    } else {
        End {
            id,
            state: State::Pending,
            reason: None,
        }
    }
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
) -> Result<Option<i64>, GoogleError> {
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
            Err(GoogleError::NotFound) => return profile_id(gmail).await,
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
async fn profile_id(gmail: &Gmail<'_>) -> Result<Option<i64>, GoogleError> {
    Ok(gmail.profile().await?.history_id.parse::<i64>().ok())
}

/// `messages.modify` each of `singles`, [`FETCH_CONCURRENCY`] at a time,
/// starting none after trouble. Returns the trouble.
async fn modify_each(
    gmail: &Gmail<'_>,
    singles: &[(String, Vec<String>, Vec<String>)],
    done: &mut Results,
) -> Option<GoogleError> {
    let mut trouble = None;
    let mut next = 0;
    let mut in_flight = FuturesOrdered::new();
    loop {
        while trouble.is_none() && in_flight.len() < FETCH_CONCURRENCY && next < singles.len() {
            let (message, add, remove) = &singles[next];
            next += 1;
            in_flight.push_back(async move { (message, gmail.modify(message, add, remove).await) });
        }
        let Some((message, result)) = in_flight.next().await else {
            break;
        };
        match result {
            Ok(_) => {
                done.written.insert(message.clone());
            }
            Err(GoogleError::NotFound) => {
                done.gone.insert(message.clone());
            }
            Err(GoogleError::Unexpected(_)) => {
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
