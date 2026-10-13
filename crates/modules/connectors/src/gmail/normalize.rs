//! Gmail's vocabulary into the contract's: labels into mailbox state,
//! flags, categories and user labels; raw MIME into addresses, headers,
//! bodies and attachments.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use mail_parser::{Address as MimeAddress, HeaderValue, MessageParser, MimeHeaders as _};

use crate::contract::{Address, Category, Direction, Headers, Label, Mailbox};

/// A message's state at Gmail, from its label ids.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Flags {
    pub mailbox: Mailbox,
    pub read: bool,
    pub starred: bool,
    pub direction: Direction,
    pub categories: Vec<Category>,
    pub labels: Vec<Label>,
}

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

/// A raw message, parsed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parsed {
    pub rfc_message_id: Option<String>,
    pub in_reply_to: Option<String>,
    pub references: Vec<String>,
    pub from: Address,
    pub to: Vec<Address>,
    pub cc: Vec<Address>,
    pub reply_to: Vec<Address>,
    pub subject: String,
    pub sent_at: Option<DateTime<Utc>>,
    pub headers: Headers,
    pub text: Option<String>,
    pub html: Option<String>,
    pub attachments: Vec<ParsedAttachment>,
}

/// An attachment's bytes and description.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedAttachment {
    pub filename: String,
    pub content_type: String,
    pub bytes: Vec<u8>,
    pub inline: bool,
}

/// What a message without a usable `From` is attributed to (RFC 2606's
/// reserved `.invalid` domain: never a real address).
pub const UNKNOWN_SENDER: &str = "unknown@sender.invalid";

/// Parse `raw` (RFC 5322). Unparseable input yields an empty message from
/// [`UNKNOWN_SENDER`] rather than an error: Gmail accepted it, so it exists.
pub fn parse(raw: &[u8]) -> Parsed {
    let Some(message) = MessageParser::default().parse(raw) else {
        return Parsed {
            rfc_message_id: None,
            in_reply_to: None,
            references: Vec::new(),
            from: unknown_sender(),
            to: Vec::new(),
            cc: Vec::new(),
            reply_to: Vec::new(),
            subject: String::new(),
            sent_at: None,
            headers: Headers::default(),
            text: None,
            html: None,
            attachments: Vec::new(),
        };
    };
    let html = message
        .html_part(0)
        .filter(|part| part.is_text_html())
        .and_then(|_| message.body_html(0))
        .map(std::borrow::Cow::into_owned);
    let attachments = message
        .attachments()
        .map(|part| {
            let content_type = part.content_type().map_or_else(
                || "application/octet-stream".to_owned(),
                |ct| match ct.subtype() {
                    Some(sub) => format!("{}/{}", ct.ctype(), sub).to_ascii_lowercase(),
                    None => ct.ctype().to_ascii_lowercase(),
                },
            );
            ParsedAttachment {
                filename: part
                    .attachment_name()
                    .map_or_else(|| "attachment".to_owned(), ToOwned::to_owned),
                content_type,
                bytes: part.contents().to_vec(),
                inline: part
                    .content_disposition()
                    .is_some_and(|d| d.ctype().eq_ignore_ascii_case("inline")),
            }
        })
        .collect();
    Parsed {
        rfc_message_id: message.message_id().map(strip_angles),
        in_reply_to: ids(message.in_reply_to()).into_iter().next(),
        references: ids(message.references()),
        from: message
            .from()
            .and_then(|from| addresses(from).into_iter().next())
            .unwrap_or_else(unknown_sender),
        to: message.to().map(addresses).unwrap_or_default(),
        cc: message.cc().map(addresses).unwrap_or_default(),
        reply_to: message.reply_to().map(addresses).unwrap_or_default(),
        subject: message.subject().unwrap_or_default().trim().to_owned(),
        sent_at: message
            .date()
            .and_then(|date| DateTime::from_timestamp(date.to_timestamp(), 0)),
        headers: Headers {
            list_id: raw_header(&message, "List-Id"),
            list_unsubscribe: raw_header(&message, "List-Unsubscribe"),
            precedence: raw_header(&message, "Precedence"),
            auto_submitted: raw_header(&message, "Auto-Submitted"),
        },
        text: message.body_text(0).map(std::borrow::Cow::into_owned),
        html,
        attachments,
    }
}

fn unknown_sender() -> Address {
    Address {
        name: None,
        address: UNKNOWN_SENDER.to_owned(),
    }
}

/// Every address of a header (groups flattened), lower-cased.
fn addresses(address: &MimeAddress<'_>) -> Vec<Address> {
    address
        .iter()
        .filter_map(|addr| {
            let address = addr.address()?.trim().to_lowercase();
            (!address.is_empty()).then(|| Address {
                name: addr
                    .name()
                    .map(str::trim)
                    .filter(|name| !name.is_empty())
                    .map(ToOwned::to_owned),
                address,
            })
        })
        .collect()
}

/// Message ids of an `In-Reply-To` or `References` header, in order,
/// without angle brackets.
fn ids(value: &HeaderValue<'_>) -> Vec<String> {
    match value {
        HeaderValue::Text(id) => vec![strip_angles(id)],
        HeaderValue::TextList(list) => list.iter().map(|id| strip_angles(id)).collect(),
        _ => Vec::new(),
    }
}

fn strip_angles(id: &str) -> String {
    id.trim()
        .trim_start_matches('<')
        .trim_end_matches('>')
        .to_owned()
}

/// A header's value as sent, unfolded and trimmed.
fn raw_header(message: &mail_parser::Message<'_>, name: &str) -> Option<String> {
    let value = message.header_raw(name)?;
    let unfolded = value
        .split(['\r', '\n'])
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    (!unfolded.is_empty()).then_some(unfolded)
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

    #[test]
    fn parses_a_multipart_message() {
        let raw = concat!(
            "From: \"Ana Silva\" <Ana@Example.com>\r\n",
            "To: Bea <bea@example.com>, team@example.com\r\n",
            "Cc: Undisclosed:;\r\n",
            "Reply-To: replies@example.com\r\n",
            "Subject: =?UTF-8?Q?Or=C3=A7amento?= de outubro\r\n",
            "Date: Tue, 13 Oct 2026 09:30:00 +0100\r\n",
            "Message-ID: <m2@example.com>\r\n",
            "In-Reply-To: <m1@example.com>\r\n",
            "References: <m0@example.com>\r\n <m1@example.com>\r\n",
            "List-Id: Clients\r\n <clients.example.com>\r\n",
            "MIME-Version: 1.0\r\n",
            "Content-Type: multipart/mixed; boundary=\"b1\"\r\n",
            "\r\n",
            "--b1\r\n",
            "Content-Type: multipart/alternative; boundary=\"b2\"\r\n",
            "\r\n",
            "--b2\r\n",
            "Content-Type: text/plain; charset=utf-8\r\n",
            "\r\n",
            "Olá, segue o orçamento.\r\n",
            "--b2\r\n",
            "Content-Type: text/html; charset=utf-8\r\n",
            "\r\n",
            "<p>Olá, segue o <b>orçamento</b>.</p>\r\n",
            "--b2--\r\n",
            "--b1\r\n",
            "Content-Type: application/pdf; name=\"orcamento.pdf\"\r\n",
            "Content-Disposition: attachment; filename=\"orcamento.pdf\"\r\n",
            "Content-Transfer-Encoding: base64\r\n",
            "\r\n",
            "JVBERi0xLjQK\r\n",
            "--b1--\r\n",
        );
        let parsed = parse(raw.as_bytes());
        assert_eq!(
            parsed.from,
            Address {
                name: Some("Ana Silva".into()),
                address: "ana@example.com".into()
            }
        );
        assert_eq!(parsed.to.len(), 2);
        assert_eq!(parsed.to[1].address, "team@example.com");
        assert!(parsed.cc.is_empty());
        assert_eq!(parsed.reply_to[0].address, "replies@example.com");
        assert_eq!(parsed.subject, "Orçamento de outubro");
        assert_eq!(
            parsed.sent_at,
            DateTime::parse_from_rfc3339("2026-10-13T08:30:00Z")
                .ok()
                .map(|d| d.to_utc())
        );
        assert_eq!(parsed.rfc_message_id.as_deref(), Some("m2@example.com"));
        assert_eq!(parsed.in_reply_to.as_deref(), Some("m1@example.com"));
        assert_eq!(parsed.references, ["m0@example.com", "m1@example.com"]);
        assert_eq!(
            parsed.headers.list_id.as_deref(),
            Some("Clients <clients.example.com>")
        );
        assert!(
            parsed
                .text
                .as_deref()
                .unwrap()
                .contains("segue o orçamento")
        );
        assert!(parsed.html.as_deref().unwrap().contains("<b>orçamento</b>"));
        assert_eq!(parsed.attachments.len(), 1);
        let pdf = &parsed.attachments[0];
        assert_eq!(pdf.filename, "orcamento.pdf");
        assert_eq!(pdf.content_type, "application/pdf");
        assert_eq!(pdf.bytes, b"%PDF-1.4\n");
        assert!(!pdf.inline);
    }

    #[test]
    fn plain_text_only_has_no_html_and_garbage_still_parses() {
        let parsed = parse(b"From: a@example.com\r\nSubject: Hi\r\n\r\nJust text.\r\n");
        assert_eq!(parsed.text.as_deref().map(str::trim), Some("Just text."));
        assert!(parsed.html.is_none());
        assert!(parsed.sent_at.is_none());
        let empty = parse(b"");
        assert_eq!(empty.from.address, UNKNOWN_SENDER);
    }
}
