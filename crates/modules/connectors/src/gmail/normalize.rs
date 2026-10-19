//! Gmail's vocabulary into the contract's: labels into mailbox state,
//! flags, categories, user labels and the provider stream.

use std::collections::{BTreeSet, HashMap};

use crate::{
    contract::{Category, Direction, Label, Mailbox, ProviderStream},
    domain::marks::{self, Marked},
    mail::Flags,
};

/// Messages the adapter does not carry: drafts and chats.
pub fn skipped(label_ids: &[String]) -> bool {
    label_ids.iter().any(|id| id == "DRAFT" || id == "CHAT")
}

/// The stream marks among `label_ids` (`user_labels` names them).
fn marks_of(
    label_ids: &[String],
    user_labels: &HashMap<String, String>,
) -> BTreeSet<ProviderStream> {
    marks::marks_in(label_ids, |id| user_labels.get(id).map(String::as_str))
}

/// The stream Gmail's category tab says: Updates, Promotions as
/// Newsletters, none for Social and Forums, otherwise Primary.
fn category_stream(label_ids: &[String]) -> Option<ProviderStream> {
    let has = |id: &str| label_ids.iter().any(|label| label == id);
    if has("CATEGORY_UPDATES") {
        Some(ProviderStream::Updates)
    } else if has("CATEGORY_PROMOTIONS") {
        Some(ProviderStream::Newsletters)
    } else if has("CATEGORY_SOCIAL") || has("CATEGORY_FORUMS") {
        None
    } else {
        Some(ProviderStream::Primary)
    }
}

/// The categories among `label_ids`, sorted.
fn categories_of(label_ids: &[String]) -> Vec<&str> {
    let mut categories: Vec<&str> = label_ids
        .iter()
        .map(String::as_str)
        .filter(|id| id.starts_with("CATEGORY_"))
        .collect();
    categories.sort_unstable();
    categories.dedup();
    categories
}

/// When a known message's category changed (labels `before`, now `now`)
/// while it kept the same stream marks, at least one: the category's
/// stream, which wins over the stale mark.
#[allow(
    clippy::option_option,
    reason = "none for no change, or the stream it says (none of the four too)"
)]
pub fn category_won(
    before: &[String],
    now: &[String],
    user_labels: &HashMap<String, String>,
) -> Option<Option<ProviderStream>> {
    let kept = marks_of(now, user_labels);
    let changed = categories_of(before) != categories_of(now);
    (changed && !kept.is_empty() && marks_of(before, user_labels) == kept)
        .then(|| category_stream(now))
}

/// The state `label_ids` describe. `user_labels` names the user labels
/// (id → name); ids not in it are system labels and are not carried, and
/// neither are stream marks, which say the provider stream of received
/// mail instead.
pub fn flags(label_ids: &[String], user_labels: &HashMap<String, String>) -> Flags {
    let has = |id: &str| label_ids.iter().any(|label| label == id);
    let in_inbox = has("INBOX");
    let mut categories: Vec<Category> = label_ids
        .iter()
        .filter_map(|id| match id.as_str() {
            "CATEGORY_UPDATES" => Some(Category::Updates),
            "CATEGORY_PROMOTIONS" => Some(Category::Promotions),
            "CATEGORY_SOCIAL" => Some(Category::Social),
            "CATEGORY_FORUMS" => Some(Category::Forums),
            _ => None,
        })
        .collect();
    categories.dedup();
    let mut labels: Vec<Label> = label_ids
        .iter()
        .filter_map(|id| {
            user_labels
                .get(id)
                .filter(|name| marks::of_name(name).is_none())
                .map(|name| Label {
                    key: id.clone(),
                    name: name.clone(),
                })
        })
        .collect();
    labels.sort_by(|a, b| a.key.cmp(&b.key));
    let direction = if has("SENT") && !in_inbox {
        Direction::Outbound
    } else {
        Direction::Inbound
    };
    let provider_stream = (direction == Direction::Inbound).then(|| {
        match marks::marked(&marks_of(label_ids, user_labels)) {
            Marked::One(stream) => Some(stream),
            Marked::Several => None,
            Marked::Unmarked => category_stream(label_ids),
        }
    });
    Flags {
        mailbox: Mailbox {
            in_inbox,
            in_spam: has("SPAM"),
            in_trash: has("TRASH"),
        },
        read: !has("UNREAD"),
        starred: has("STARRED"),
        direction,
        categories,
        labels,
        provider_stream,
        stream_mark_stale: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(list: &[&str]) -> Vec<String> {
        list.iter().map(|&s| s.to_owned()).collect()
    }

    #[test]
    fn labels_map_to_state() {
        let names = HashMap::from([("Label_7".to_owned(), "Clients".to_owned())]);
        let flags = flags(
            &ids(&[
                "INBOX",
                "UNREAD",
                "STARRED",
                "CATEGORY_UPDATES",
                "Label_7",
                "IMPORTANT",
            ]),
            &names,
        );
        assert_eq!(
            flags.mailbox,
            Mailbox {
                in_inbox: true,
                in_spam: false,
                in_trash: false
            }
        );
        assert!(!flags.read && flags.starred);
        assert_eq!(flags.direction, Direction::Inbound);
        assert_eq!(flags.categories, [Category::Updates]);
        assert_eq!(
            flags.labels,
            [Label {
                key: "Label_7".into(),
                name: "Clients".into()
            }]
        );

        assert_eq!(flags.provider_stream, Some(Some(ProviderStream::Updates)));

        let sent = super::flags(&ids(&["SENT"]), &HashMap::new());
        assert_eq!(sent.provider_stream, None);
        assert_eq!(sent.direction, Direction::Outbound);
        assert!(sent.read && !sent.mailbox.in_inbox);
        let to_self = super::flags(&ids(&["SENT", "INBOX"]), &HashMap::new());
        assert_eq!(to_self.direction, Direction::Inbound);
        let trash = super::flags(&ids(&["TRASH", "SPAM"]), &HashMap::new());
        assert!(trash.mailbox.in_trash && trash.mailbox.in_spam);
        assert!(skipped(&ids(&["DRAFT"])) && skipped(&ids(&["CHAT"])));
        assert!(!skipped(&ids(&["INBOX"])));
    }

    #[test]
    fn a_mark_is_not_a_label() {
        let names = HashMap::from([
            ("Label_7".to_owned(), "Clients".to_owned()),
            ("Label_3".to_owned(), "Campbooks/Receipts".to_owned()),
            ("Label_4".to_owned(), "campbooks/newsletters".to_owned()),
        ]);
        let receipt = flags(
            &ids(&["INBOX", "CATEGORY_UPDATES", "Label_3", "Label_7"]),
            &names,
        );
        assert_eq!(
            receipt.labels,
            [Label {
                key: "Label_7".into(),
                name: "Clients".into()
            }]
        );
        assert_eq!(
            receipt.provider_stream,
            Some(Some(ProviderStream::Receipts))
        );
        let stream = |labels: &[&str]| flags(&ids(labels), &names).provider_stream;
        assert_eq!(stream(&["INBOX"]), Some(Some(ProviderStream::Primary)));
        assert_eq!(
            stream(&["INBOX", "CATEGORY_PERSONAL"]),
            Some(Some(ProviderStream::Primary))
        );
        assert_eq!(
            stream(&["INBOX", "CATEGORY_PROMOTIONS"]),
            Some(Some(ProviderStream::Newsletters))
        );
        assert_eq!(stream(&["INBOX", "CATEGORY_SOCIAL"]), Some(None));
        assert_eq!(stream(&["INBOX", "CATEGORY_FORUMS"]), Some(None));
        // Several marks: none trusted.
        assert_eq!(stream(&["INBOX", "Label_3", "Label_4"]), Some(None));
        // A mark wins over the category.
        assert_eq!(
            stream(&["INBOX", "CATEGORY_PROMOTIONS", "Label_4"]),
            Some(Some(ProviderStream::Newsletters))
        );
    }

    #[test]
    fn a_tab_change_on_a_marked_message_wins_over_the_mark() {
        let names = HashMap::from([("Label_3".to_owned(), "Campbooks/Receipts".to_owned())]);
        let receipt = ids(&["INBOX", "CATEGORY_UPDATES", "Label_3"]);
        let dragged = ids(&["INBOX", "CATEGORY_PROMOTIONS", "Label_3"]);
        assert_eq!(
            category_won(&receipt, &dragged, &names),
            Some(Some(ProviderStream::Newsletters))
        );
        // The mark taken off as well: the read says so itself.
        let unmarked = ids(&["INBOX", "CATEGORY_PROMOTIONS"]);
        assert_eq!(category_won(&receipt, &unmarked, &names), None);
        // No category change, or no mark: nothing to say.
        let read = ids(&["CATEGORY_UPDATES", "Label_3"]);
        assert_eq!(category_won(&receipt, &read, &names), None);
        assert_eq!(
            category_won(
                &ids(&["CATEGORY_UPDATES"]),
                &ids(&["CATEGORY_PROMOTIONS"]),
                &names
            ),
            None
        );
    }
}
