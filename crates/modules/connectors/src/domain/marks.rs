//! Stream marks: the provider labels that carry a Campbooks stream where
//! the provider has no place of its own for it (design of the
//! `provider-stream-labels` change).
//!
//! | Stream      | Gmail                                     | Zoho                    |
//! |-------------|-------------------------------------------|-------------------------|
//! | Primary     | `CATEGORY_PERSONAL`                       | no mark                 |
//! | Updates     | `CATEGORY_UPDATES`                        | `Campbooks/Updates`     |
//! | Receipts    | `CATEGORY_UPDATES` + `Campbooks/Receipts` | `Campbooks/Receipts`    |
//! | Newsletters | `CATEGORY_PROMOTIONS`                     | `Campbooks/Newsletters` |
//!
//! The names are a protocol, the same in every language, so the connector
//! finds a mark again by its name (exact, case-insensitive) in the
//! provider's label listing. A move asks for a mark by its token
//! (`mark:<stream>`); the push turns the token into the provider's label,
//! creating it the first time one is needed. Marks are never carried as
//! labels: a message's marks say its provider stream.

use std::collections::{BTreeMap, BTreeSet};

use super::{
    outbound::{Backlog, LabelChange},
    push::Plan,
};
use crate::contract::ProviderStream;

/// Each stream's mark name. `Campbooks/Primary` is reserved: read as
/// Primary when found, never written.
pub const NAMES: [(ProviderStream, &str); 4] = [
    (ProviderStream::Primary, "Campbooks/Primary"),
    (ProviderStream::Updates, "Campbooks/Updates"),
    (ProviderStream::Receipts, "Campbooks/Receipts"),
    (ProviderStream::Newsletters, "Campbooks/Newsletters"),
];

/// The prefix of a mark's token in an outbound change.
const TOKEN: &str = "mark:";

/// The mark name of `stream`.
pub fn name(stream: ProviderStream) -> &'static str {
    NAMES
        .iter()
        .find(|(s, _)| *s == stream)
        .map_or("Campbooks/Primary", |(_, name)| name)
}

/// The stream a label named `label` marks, if it is a mark.
pub fn of_name(label: &str) -> Option<ProviderStream> {
    NAMES
        .iter()
        .find(|(_, name)| name.eq_ignore_ascii_case(label))
        .map(|(stream, _)| *stream)
}

/// The token of `stream`'s mark in an outbound change.
pub fn token(stream: ProviderStream) -> String {
    format!("{TOKEN}{}", stream.as_str())
}

/// The stream of a mark's token.
pub fn of_token(token: &str) -> Option<ProviderStream> {
    token.strip_prefix(TOKEN).and_then(ProviderStream::parse)
}

/// Gmail's place for `stream`: its category, and its mark where the
/// category alone does not say it.
pub const fn gmail_place(stream: ProviderStream) -> (&'static str, Option<ProviderStream>) {
    match stream {
        ProviderStream::Primary => ("CATEGORY_PERSONAL", None),
        ProviderStream::Updates => ("CATEGORY_UPDATES", None),
        ProviderStream::Receipts => ("CATEGORY_UPDATES", Some(ProviderStream::Receipts)),
        ProviderStream::Newsletters => ("CATEGORY_PROMOTIONS", None),
    }
}

/// Zoho's mark for `stream`: none for Primary (the mark-free inbox).
pub const fn zoho_mark(stream: ProviderStream) -> Option<ProviderStream> {
    match stream {
        ProviderStream::Primary => None,
        other => Some(other),
    }
}

/// A move's labels: `add` and `remove` (the provider's native place), then
/// `mark` on and every other stream mark off.
pub fn with_mark(
    mut add: Vec<String>,
    mut remove: Vec<String>,
    mark: Option<ProviderStream>,
) -> LabelChange {
    if let Some(mark) = mark {
        add.push(token(mark));
    }
    remove.extend(
        ProviderStream::ALL
            .into_iter()
            .filter(|s| Some(*s) != mark)
            .map(token),
    );
    (add, remove)
}

/// The streams the marks among `labels` name: a label whose name
/// (`name_of`) is a mark's, or a mark's token (a change still waiting).
pub fn marks_in<'a, 'n>(
    labels: impl IntoIterator<Item = &'a String>,
    name_of: impl Fn(&str) -> Option<&'n str>,
) -> BTreeSet<ProviderStream> {
    labels
        .into_iter()
        .filter_map(|label| of_token(label).or_else(|| name_of(label).and_then(of_name)))
        .collect()
}

/// What a message's marks say of its stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Marked {
    /// No mark: its native place says.
    Unmarked,
    /// One stream.
    One(ProviderStream),
    /// Marks of several streams: none can be trusted.
    Several,
}

/// What `marks` say.
pub fn marked(marks: &BTreeSet<ProviderStream>) -> Marked {
    let mut each = marks.iter();
    match (each.next(), each.next()) {
        (None, _) => Marked::Unmarked,
        (Some(stream), None) => Marked::One(*stream),
        (Some(_), Some(_)) => Marked::Several,
    }
}

/// The provider's label id of each mark it has.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MarkIds(BTreeMap<ProviderStream, String>);

impl MarkIds {
    /// The marks among `labels` (id, name). When two labels have a mark's
    /// name (differing in case), the smaller id is the mark.
    pub fn from_labels<'a>(labels: impl IntoIterator<Item = (&'a str, &'a str)>) -> Self {
        let mut ids: BTreeMap<ProviderStream, String> = BTreeMap::new();
        for (id, label) in labels {
            let Some(stream) = of_name(label) else {
                continue;
            };
            ids.entry(stream)
                .and_modify(|held| {
                    if id < held.as_str() {
                        id.clone_into(held);
                    }
                })
                .or_insert_with(|| id.to_owned());
        }
        Self(ids)
    }

    /// The id of `stream`'s mark.
    pub fn get(&self, stream: ProviderStream) -> Option<&str> {
        self.0.get(&stream).map(String::as_str)
    }

    /// `stream`'s mark is the label `id`.
    pub fn insert(&mut self, stream: ProviderStream, id: String) {
        self.0.insert(stream, id);
    }
}

/// The marks the winning changes of `backlog` put on that the provider has
/// none of yet (`ids`): what the push creates first.
pub fn missing(backlog: &Backlog, ids: &MarkIds) -> BTreeSet<ProviderStream> {
    Plan::merge(backlog)
        .wants
        .values()
        .flat_map(BTreeMap::iter)
        .filter(|(_, want)| want.present)
        .filter_map(|(label, _)| of_token(label))
        .filter(|stream| ids.get(*stream).is_none())
        .collect()
}

/// `labels` with every mark's token as the provider's label (`label` makes
/// one from an id). A mark the provider does not have is kept as its token
/// when `keep` (an add, still wanted) and dropped otherwise (a remove:
/// nothing to take off).
fn resolved(
    labels: &[String],
    ids: &MarkIds,
    label: &impl Fn(&str) -> String,
    keep: bool,
) -> Vec<String> {
    labels
        .iter()
        .filter_map(|l| match of_token(l) {
            None => Some(l.clone()),
            Some(stream) => match ids.get(stream) {
                Some(id) => Some(label(id)),
                None => keep.then(|| l.clone()),
            },
        })
        .collect()
}

/// `changes` with marks' tokens as the provider's labels (see
/// [`resolved`]).
pub fn resolve_changes(
    changes: &[LabelChange],
    ids: &MarkIds,
    label: impl Fn(&str) -> String,
) -> Vec<LabelChange> {
    changes
        .iter()
        .map(|(add, remove)| {
            (
                resolved(add, ids, &label, true),
                resolved(remove, ids, &label, false),
            )
        })
        .collect()
}

/// `backlog` with marks' tokens as the provider's labels, in its pending
/// changes and in the module's own earlier writes.
pub fn resolve(backlog: &Backlog, ids: &MarkIds, label: impl Fn(&str) -> String) -> Backlog {
    let mut out = backlog.clone();
    for change in &mut out.pending {
        change.add = resolved(&change.add, ids, &label, true);
        change.remove = resolved(&change.remove, ids, &label, false);
    }
    for writes in out.own.values_mut() {
        for write in writes {
            write.labels = resolved(&write.labels, ids, &label, false);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    #[test]
    fn marks_are_found_by_name_whatever_the_case() {
        assert_eq!(
            of_name("Campbooks/Receipts"),
            Some(ProviderStream::Receipts)
        );
        assert_eq!(
            of_name("campbooks/receipts"),
            Some(ProviderStream::Receipts)
        );
        assert_eq!(of_name("CAMPBOOKS/UPDATES"), Some(ProviderStream::Updates));
        assert_eq!(of_name("Campbooks/Recibos"), None);
        assert_eq!(of_name("Receipts"), None);
        assert_eq!(of_name(" Campbooks/Receipts"), None);
        assert_eq!(name(ProviderStream::Newsletters), "Campbooks/Newsletters");
        assert_eq!(
            of_token(&token(ProviderStream::Receipts)),
            Some(ProviderStream::Receipts)
        );
        assert_eq!(of_token("Label_7"), None);
        let ids = MarkIds::from_labels([
            ("Label_9", "campbooks/receipts"),
            ("Label_3", "Campbooks/Receipts"),
            ("Label_7", "Clients"),
        ]);
        assert_eq!(ids.get(ProviderStream::Receipts), Some("Label_3"));
        assert!(ids.get(ProviderStream::Updates).is_none());
    }

    #[test]
    fn a_messages_marks_say_its_stream() {
        let names: HashMap<String, String> = [
            ("Label_3".to_owned(), "Campbooks/Receipts".to_owned()),
            ("Label_4".to_owned(), "Campbooks/Updates".to_owned()),
            ("Label_7".to_owned(), "Clients".to_owned()),
        ]
        .into();
        let name_of = |id: &str| names.get(id).map(String::as_str);
        let of = |labels: &[&str]| {
            let labels: Vec<String> = labels.iter().map(|&l| l.to_owned()).collect();
            marked(&marks_in(&labels, name_of))
        };
        assert_eq!(of(&["INBOX", "Label_7"]), Marked::Unmarked);
        assert_eq!(
            of(&["Label_3", "Label_7"]),
            Marked::One(ProviderStream::Receipts)
        );
        assert_eq!(
            of(&["mark:newsletters"]),
            Marked::One(ProviderStream::Newsletters)
        );
        assert_eq!(of(&["Label_3", "Label_4"]), Marked::Several);
        assert_eq!(
            of(&["Label_3", "mark:receipts"]),
            Marked::One(ProviderStream::Receipts)
        );
    }

    #[test]
    fn tokens_become_the_providers_labels() {
        let mut ids = MarkIds::default();
        ids.insert(ProviderStream::Receipts, "Label_3".to_owned());
        let (add, remove) = with_mark(
            vec!["CATEGORY_UPDATES".to_owned()],
            vec!["CATEGORY_PERSONAL".to_owned()],
            Some(ProviderStream::Updates),
        );
        assert_eq!(add, ["CATEGORY_UPDATES", "mark:updates"]);
        assert_eq!(
            remove,
            [
                "CATEGORY_PERSONAL",
                "mark:primary",
                "mark:receipts",
                "mark:newsletters"
            ]
        );
        let [(add, remove)] = resolve_changes(&[(add, remove)], &ids, |id| format!("label:{id}"))
            .try_into()
            .unwrap();
        // The missing Updates mark stays wanted; the missing ones are not
        // taken off.
        assert_eq!(add, ["CATEGORY_UPDATES", "mark:updates"]);
        assert_eq!(remove, ["CATEGORY_PERSONAL", "label:Label_3"]);
    }
}
