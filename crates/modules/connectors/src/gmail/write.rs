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

use std::collections::BTreeMap;

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

/// Gmail quota units a push of `messages` messages may spend: a profile
/// call, the history pages and one modify per message.
pub fn units(messages: usize) -> u32 {
    let messages = u32::try_from(messages).unwrap_or(u32::MAX);
    1 + 2 * u32::try_from(HISTORY_PAGES).unwrap_or(4) + messages.saturating_mul(5)
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
) -> Option<ProviderError> {
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
                Err(ProviderError::NotFound | ProviderError::Unexpected(_)) => {
                    singles.extend(one_by_one(chunk));
                }
                Err(trouble) => return Some(trouble),
            }
        }
    }
    modify_each(gmail, &singles, done).await
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

/// `messages.modify` each of `singles`, [`FETCH_CONCURRENCY`] at a time,
/// starting none after trouble. Returns the trouble.
async fn modify_each(
    gmail: &Gmail<'_>,
    singles: &[(String, Vec<String>, Vec<String>)],
    done: &mut Results,
) -> Option<ProviderError> {
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
