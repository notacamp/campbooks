//! Gmail's vocabulary into the contract's: labels into mailbox state,
//! flags, categories and user labels.

use std::collections::HashMap;

use crate::{
    contract::{Category, Direction, Label, Mailbox},
    mail::Flags,
};

/// Messages the adapter does not carry: drafts and chats.
pub fn skipped(label_ids: &[String]) -> bool {
    label_ids.iter().any(|id| id == "DRAFT" || id == "CHAT")
}

/// The state `label_ids` describe. `user_labels` names the user labels
/// (id → name); ids not in it are system labels and are not carried.
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
            user_labels.get(id).map(|name| Label {
                key: id.clone(),
                name: name.clone(),
            })
        })
        .collect();
    labels.sort_by(|a, b| a.key.cmp(&b.key));
    Flags {
        mailbox: Mailbox {
            in_inbox,
            in_spam: has("SPAM"),
            in_trash: has("TRASH"),
        },
        read: !has("UNREAD"),
        starred: has("STARRED"),
        direction: if has("SENT") && !in_inbox {
            Direction::Outbound
        } else {
            Direction::Inbound
        },
        categories,
        labels,
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

        let sent = super::flags(&ids(&["SENT"]), &HashMap::new());
        assert_eq!(sent.direction, Direction::Outbound);
        assert!(sent.read && !sent.mailbox.in_inbox);
        let to_self = super::flags(&ids(&["SENT", "INBOX"]), &HashMap::new());
        assert_eq!(to_self.direction, Direction::Inbound);
        let trash = super::flags(&ids(&["TRASH", "SPAM"]), &HashMap::new());
        assert!(trash.mailbox.in_trash && trash.mailbox.in_spam);
        assert!(skipped(&ids(&["DRAFT"])) && skipped(&ids(&["CHAT"])));
        assert!(!skipped(&ids(&["INBOX"])));
    }
}
