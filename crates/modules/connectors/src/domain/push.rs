//! Planning a stream's push of pending outbound changes, whatever the
//! provider: merge them (per message and label the latest recorded change
//! wins; earlier ones end `superseded`, `replaced`), make each whole or not
//! at all, group messages with the same change, and say how each change
//! ended. The adapters (`gmail/write.rs`, `zoho/write.rs`) do the provider's
//! calls and mark what a later writer at the provider overtook.

use std::collections::{BTreeMap, HashMap, HashSet};

use uuid::Uuid;

use super::outbound::{Backlog, End, LabelChange, MAX_ATTEMPTS, Outcome, State};
use crate::provider::ProviderError;

/// What a push did, and the provider trouble that stopped it, if any.
#[derive(Debug, Default)]
pub struct Push {
    /// To record.
    pub outcome: Outcome,
    /// The provider failed in a way the run's backoff handles.
    pub trouble: Option<ProviderError>,
}

/// One label's wanted state on one message: the change that won it.
#[derive(Debug, Clone)]
pub struct Want {
    pub present: bool,
    pub row: Uuid,
    pub stamp: Option<i64>,
}

/// How a won (message, label) ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fate {
    Write,
    Overtaken,
    Gone,
}

/// Wanted label states, per message then label.
pub type Wants = BTreeMap<String, BTreeMap<String, Want>>;

/// How each wanted (message, label) ends.
pub type Fates = BTreeMap<(String, String), Fate>;

/// The merged changes of a backlog and how each is to end.
pub struct Plan {
    /// The winning change per message and label.
    pub wants: Wants,
    /// How each (message, label) is to end.
    pub fates: Fates,
    /// The (message, label) pairs each change won.
    won: HashMap<Uuid, Vec<(String, String)>>,
    /// Changes that would bring a message in the trash or spam back into
    /// the inbox or a category without taking it out of the trash or spam
    /// (an untrash or "not spam" does): skipped.
    in_trash: HashSet<Uuid>,
}

/// What the writes did, per message.
#[derive(Debug, Default)]
pub struct Results {
    /// Messages the provider changed.
    pub written: HashSet<String>,
    /// Messages the provider refused to change.
    pub refused: HashSet<String>,
    /// Messages the provider no longer has.
    pub gone: HashSet<String>,
    /// Messages a change would put back in the inbox although they are not
    /// the member's to move there (their own sent mail, at Zoho): that part
    /// is not made.
    pub unmovable: HashSet<String>,
}

impl Plan {
    /// Merge: per message and label, the latest change wins. Messages the
    /// index does not carry are gone.
    pub fn merge(backlog: &Backlog) -> Self {
        let mut wants = Wants::new();
        let mut in_trash = HashSet::new();
        for change in &backlog.pending {
            // In the trash or spam, and not taken out of every one of them
            // by this change (an untrash or a "not spam" is).
            let trashed = backlog
                .labels
                .get(&change.provider_message_id)
                .is_some_and(|labels| {
                    labels
                        .iter()
                        .any(|l| (l == "TRASH" || l == "SPAM") && !change.remove.contains(l))
                });
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
    pub fn atomic(&mut self) {
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
    pub fn change_of(&self, message: &str) -> LabelChange {
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
    pub fn groups(&self) -> BTreeMap<LabelChange, Vec<String>> {
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
    pub fn ends(&self, backlog: &Backlog, done: &Results, outcome: &mut Outcome) {
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
            let only_inbox = !pairs.is_empty() && pairs.iter().all(|(_, label)| label == "INBOX");
            let end = if pairs.is_empty() {
                ended(State::Superseded, Some("replaced"))
            } else if done.unmovable.contains(message) && only_inbox {
                // The provider's state stands: Campbooks is told so.
                outcome.reemit.push(message.clone());
                ended(State::Skipped, Some("not_movable"))
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

/// A change the push did not get to because of trouble: pending again, or
/// failed once it has met trouble [`MAX_ATTEMPTS`] times.
pub fn retry_or_give_up(id: Uuid, attempts: i32, reemit: &mut Vec<String>, message: &str) -> End {
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
