//! An outgoing message document into RFC 5322 bytes, for
//! `users.messages.send`. Gmail reads the recipients (Bcc included) from the
//! headers and strips `Bcc` from what it delivers.

use mail_builder::{
    MessageBuilder,
    headers::{address::Address as MimeAddress, date::Date, message_id::MessageId},
};

use crate::contract::{Address, OutgoingMessage};

/// A header value with every control character (a line break would start
/// a new header) turned into a space.
fn clean(value: &str) -> String {
    value
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect::<String>()
        .trim()
        .to_owned()
}

fn address(address: &Address) -> MimeAddress<'static> {
    let email = clean(&address.address);
    match address.name.as_deref().map(clean).filter(|n| !n.is_empty()) {
        Some(name) => MimeAddress::from((name, email)),
        None => MimeAddress::from(email),
    }
}

fn list(addresses: &[Address]) -> MimeAddress<'static> {
    MimeAddress::new_list(addresses.iter().map(address).collect())
}

/// The message `document` describes, with `attachments` (their bytes, in
/// the document's order).
pub fn rfc5322(document: &OutgoingMessage, attachments: Vec<Vec<u8>>) -> Vec<u8> {
    let mut message = MessageBuilder::new()
        .from(address(&document.from))
        .subject(clean(&document.subject))
        .date(Date::new(document.date.timestamp()))
        .message_id(MessageId::new(clean(&document.rfc_message_id)));
    if !document.to.is_empty() {
        message = message.to(list(&document.to));
    }
    if !document.cc.is_empty() {
        message = message.cc(list(&document.cc));
    }
    if !document.bcc.is_empty() {
        message = message.bcc(list(&document.bcc));
    }
    if let Some(parent) = &document.in_reply_to {
        message = message.in_reply_to(MessageId::new(clean(parent)));
    }
    if !document.references.is_empty() {
        let references: Vec<String> = document.references.iter().map(|r| clean(r)).collect();
        message = message.references(MessageId::from(references));
    }
    message = message.text_body(document.text.clone());
    if let Some(html) = &document.html {
        message = message.html_body(html.clone());
    }
    for (attachment, bytes) in document.attachments.iter().zip(attachments) {
        message = message.attachment(
            clean(&attachment.content_type),
            clean(&attachment.filename),
            bytes,
        );
    }
    message
        .write_to_vec()
        .unwrap_or_else(|e| unreachable!("writing to memory cannot fail: {e}"))
}

#[cfg(test)]
mod tests {
    use campbooks_kernel::blobs::BlobRef;
    use chrono::{DateTime, Utc};

    use super::*;
    use crate::{contract::OutgoingAttachment, domain::model::InboxId, mail::parse as normalize};

    fn person(name: Option<&str>, address: &str) -> Address {
        Address {
            name: name.map(ToOwned::to_owned),
            address: address.to_owned(),
        }
    }

    fn document() -> OutgoingMessage {
        OutgoingMessage {
            version: 1,
            inbox: InboxId::new(),
            rfc_message_id: "0192-send@example.com".into(),
            from: person(Some("Sam Rivers"), "sam@example.com"),
            to: vec![person(Some("Maya Okafor"), "maya@northwind-studio.example")],
            cc: vec![person(Some("Joana Simões"), "joana@example.com")],
            bcc: vec![person(None, "archive@example.com")],
            subject: "Re: Orçamento\r\nBcc: evil@example.com".into(),
            date: DateTime::<Utc>::from_timestamp(1_760_000_000, 0).unwrap(),
            in_reply_to: Some("q4-plan-1@studio.example".into()),
            references: vec![
                "q4-plan-0@studio.example".into(),
                "q4-plan-1@studio.example".into(),
            ],
            thread_key: Some("thr-1".into()),
            text: "**Done**, see you Thursday.\n\nOn Sun, Maya wrote:\n> Could you?".into(),
            html: Some("<div><p><strong>Done</strong>, see you Thursday.</p></div>".into()),
            attachments: vec![OutgoingAttachment {
                filename: "plano-q4.pdf".into(),
                content_type: "application/pdf".into(),
                blob: BlobRef {
                    hash: "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08"
                        .parse()
                        .unwrap(),
                    size: 9,
                    content_type: "application/pdf".into(),
                },
            }],
        }
    }

    #[test]
    fn the_message_parses_back_to_the_document() {
        let raw = rfc5322(&document(), vec![b"%PDF-1.7".to_vec()]);
        let parsed = normalize::parse(&raw, crate::mail::Receiver::Gmail);
        assert_eq!(
            parsed.rfc_message_id.as_deref(),
            Some("0192-send@example.com")
        );
        assert_eq!(
            parsed.in_reply_to.as_deref(),
            Some("q4-plan-1@studio.example")
        );
        assert_eq!(
            parsed.references,
            ["q4-plan-0@studio.example", "q4-plan-1@studio.example"]
        );
        assert_eq!(parsed.from.address, "sam@example.com");
        assert_eq!(parsed.to[0].address, "maya@northwind-studio.example");
        assert_eq!(parsed.cc[0].name.as_deref(), Some("Joana Simões"));
        // A line break in the subject cannot add a header.
        assert_eq!(parsed.subject, "Re: Orçamento  Bcc: evil@example.com");
        assert!(parsed.text.as_deref().unwrap().contains("> Could you?"));
        assert!(
            parsed
                .html
                .as_deref()
                .unwrap()
                .contains("<strong>Done</strong>")
        );
        assert_eq!(parsed.attachments.len(), 1);
        assert_eq!(parsed.attachments[0].filename, "plano-q4.pdf");
        assert_eq!(parsed.attachments[0].bytes, b"%PDF-1.7");
        let text = String::from_utf8_lossy(&raw);
        assert!(
            text.contains("Bcc: <archive@example.com>")
                || text.contains("Bcc: archive@example.com")
        );
        assert_eq!(text.matches("\nBcc:").count(), 1);
    }
}
