//! Raw MIME into the contract's addresses, headers, bodies and attachments,
//! whichever provider the message came from.

use chrono::{DateTime, Utc};
use mail_parser::{Address as MimeAddress, HeaderValue, MessageParser, MimeHeaders as _};

use super::Receiver;
use crate::contract::{Address, Headers};

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
    /// Its `Content-ID`, without angle brackets: what `cid:` images name.
    pub content_id: Option<String>,
}

/// Longest `Content-ID` kept, in characters (the `MessageChangeBatch` v1
/// limit); a longer one names no image anyone wrote, and is left out.
pub const MAX_CONTENT_ID_CHARS: usize = 255;

/// What a message without a usable `From` is attributed to (RFC 2606's
/// reserved `.invalid` domain: never a real address).
pub const UNKNOWN_SENDER: &str = "unknown@sender.invalid";

/// Parse `raw` (RFC 5322), received by `receiver`. Unparseable input yields
/// an empty message from [`UNKNOWN_SENDER`] rather than an error: the
/// provider accepted it, so it exists.
pub fn parse(raw: &[u8], receiver: Receiver) -> Parsed {
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
    // Only a text part the message really has: with none, the parser would
    // make one out of the HTML with its own reading, which loses the words
    // of HTML with no `<body>` tag (campbooks-cloud#218). The message is
    // then its HTML alone, and conversations reads it as text itself.
    let text = message
        .text_part(0)
        .filter(|part| !part.is_text_html())
        .and_then(|_| message.body_text(0))
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
                content_id: part
                    .content_id()
                    .map(strip_angles)
                    .filter(|id| !id.is_empty() && id.chars().count() <= MAX_CONTENT_ID_CHARS),
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
            authentication_results: authentication_results(&message, receiver),
        },
        text,
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
    unfold(message.header_raw(name)?)
}

fn unfold(value: &str) -> Option<String> {
    let unfolded = value
        .split(['\r', '\n'])
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    (!unfolded.is_empty()).then_some(unfolded)
}

/// The receiving provider's own `Authentication-Results`: the topmost one
/// (each hop adds its own above the others), kept only when its authserv-id
/// is `receiver`'s. A sender can write any such header into a message, but
/// not above the one its receiver adds; anything else is no result.
fn authentication_results(
    message: &mail_parser::Message<'_>,
    receiver: Receiver,
) -> Option<String> {
    // Every header, in order, raw: one that is not UTF-8 is still the
    // topmost, and makes the results absent rather than letting the next one
    // down take its place.
    let topmost = message.headers().iter().find(|header| {
        header
            .name
            .as_str()
            .eq_ignore_ascii_case("Authentication-Results")
    })?;
    let bytes = message
        .raw_message()
        .get(topmost.offset_start as usize..topmost.offset_end as usize)?;
    let value = unfold(std::str::from_utf8(bytes).ok()?)?;
    let authserv_id = value
        .split(';')
        .next()?
        .split_whitespace()
        .next()?
        .to_ascii_lowercase();
    receiver
        .authserv_ids()
        .contains(&authserv_id.as_str())
        .then_some(value)
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let parsed = parse(raw.as_bytes(), Receiver::Gmail);
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
        assert_eq!(pdf.content_id, None);
    }

    #[test]
    fn inline_parts_keep_their_content_id() {
        let raw = concat!(
            "From: Ana <ana@example.com>\r\n",
            "Subject: Logo\r\n",
            "MIME-Version: 1.0\r\n",
            "Content-Type: multipart/related; boundary=\"r1\"\r\n",
            "\r\n",
            "--r1\r\n",
            "Content-Type: text/html; charset=utf-8\r\n",
            "\r\n",
            "<p><img src=\"cid:logo@example.com\"></p>\r\n",
            "--r1\r\n",
            "Content-Type: image/png; name=\"logo.png\"\r\n",
            "Content-Disposition: inline; filename=\"logo.png\"\r\n",
            "Content-ID: <logo@example.com>\r\n",
            "Content-Transfer-Encoding: base64\r\n",
            "\r\n",
            "iVBORw0KGgo=\r\n",
            "--r1--\r\n",
        );
        let parsed = parse(raw.as_bytes(), Receiver::Gmail);
        assert_eq!(parsed.attachments.len(), 1);
        let logo = &parsed.attachments[0];
        assert!(logo.inline);
        assert_eq!(logo.content_id.as_deref(), Some("logo@example.com"));

        let long = raw.replace(
            "logo@example.com>",
            &format!("{}@example.com>", "x".repeat(300)),
        );
        assert_eq!(
            parse(long.as_bytes(), Receiver::Gmail).attachments[0].content_id,
            None
        );
        assert_eq!(logo.content_type, "image/png");
    }

    #[test]
    fn html_alone_gets_no_text_made_up() {
        // As the demo Gmail and some mailers send it: one `text/html` part
        // opening with a head, no `<body>` tag (campbooks-cloud#218).
        let raw = concat!(
            "From: Quay Office <office@quay-moorings.example>\r\n",
            "Subject: Your berth for next season\r\n",
            "MIME-Version: 1.0\r\n",
            "Content-Type: text/html; charset=utf-8\r\n",
            "Content-Transfer-Encoding: base64\r\n",
            "\r\n",
            "PGhlYWQ+PG1ldGEgY2hhcnNldD0idXRmLTgiPjxwPkhlbGxvIFNhbSw8L3A+PHA+WW91ciBiZXJ0aCBh\r\n",
            "dCB0aGUgaGFyYm91cmxpZ2h0IHBvbnRvb24gaXMga2VwdC48L3A+\r\n",
        );
        let parsed = parse(raw.as_bytes(), Receiver::Gmail);
        assert_eq!(parsed.text, None);
        assert!(
            parsed
                .html
                .as_deref()
                .is_some_and(|html| html.contains("harbourlight pontoon")),
            "{:?}",
            parsed.html
        );
        // Related parts around the HTML: still no text made up.
        let related = parse(
            concat!(
                "From: a@example.com\r\n",
                "MIME-Version: 1.0\r\n",
                "Content-Type: multipart/related; boundary=\"r\"\r\n\r\n",
                "--r\r\nContent-Type: text/html; charset=utf-8\r\n\r\n",
                "<p>Only HTML</p>\r\n--r--\r\n",
            )
            .as_bytes(),
            Receiver::Gmail,
        );
        assert_eq!(related.text, None);
        assert!(related.html.is_some());
    }

    #[test]
    fn plain_text_only_has_no_html_and_garbage_still_parses() {
        let parsed = parse(
            b"From: a@example.com\r\nSubject: Hi\r\n\r\nJust text.\r\n",
            Receiver::Gmail,
        );
        assert_eq!(parsed.text.as_deref().map(str::trim), Some("Just text."));
        assert!(parsed.html.is_none());
        assert!(parsed.sent_at.is_none());
        let empty = parse(b"", Receiver::Gmail);
        assert_eq!(empty.from.address, UNKNOWN_SENDER);
    }

    fn with_results(results: &[&str]) -> String {
        let mut raw = String::new();
        for value in results {
            raw.push_str("Authentication-Results: ");
            raw.push_str(value);
            raw.push_str("\r\n");
        }
        raw.push_str(concat!(
            "From: GitHub <noreply@github.com>\r\n",
            "To: sam@example.com\r\n",
            "Subject: [example-org/demo-shop] Security alert\r\n",
            "\r\n",
            "Dependabot found a vulnerability.\r\n",
        ));
        raw
    }

    const GMAIL_PASS: &str = "mx.google.com;\r\n       dkim=pass header.i=@github.com header.s=pf2023;\r\n       \
                              dmarc=pass (p=REJECT sp=REJECT dis=NONE) header.from=github.com";

    #[test]
    fn the_receivers_own_authentication_results_are_kept_unfolded() {
        let parsed = parse(with_results(&[GMAIL_PASS]).as_bytes(), Receiver::Gmail);
        assert_eq!(
            parsed.headers.authentication_results.as_deref(),
            Some(
                "mx.google.com; dkim=pass header.i=@github.com header.s=pf2023; \
                 dmarc=pass (p=REJECT sp=REJECT dis=NONE) header.from=github.com"
            )
        );
    }

    #[test]
    fn results_a_sender_wrote_are_not_trusted() {
        // Another server's results on top: not the receiver's own.
        let forged_on_top = with_results(&[
            "mail.attacker.example; dmarc=pass header.from=github.com",
            GMAIL_PASS,
        ]);
        assert_eq!(
            parse(forged_on_top.as_bytes(), Receiver::Gmail)
                .headers
                .authentication_results,
            None
        );
        // Gmail's id read by a Zoho mailbox: written by someone else.
        assert_eq!(
            parse(with_results(&[GMAIL_PASS]).as_bytes(), Receiver::Zoho)
                .headers
                .authentication_results,
            None
        );
        // No results at all.
        assert_eq!(
            parse(with_results(&[]).as_bytes(), Receiver::Gmail)
                .headers
                .authentication_results,
            None
        );
    }

    #[test]
    fn a_zoho_mailbox_trusts_no_results_yet() {
        // Until a real Zoho message shows which header Zoho adds, a header
        // naming Zoho's servers may be one the sender wrote.
        let raw = with_results(&[
            "mx.zohomail.com; dkim=pass; spf=pass; dmarc=pass header.from=github.com",
        ]);
        assert_eq!(
            parse(raw.as_bytes(), Receiver::Zoho)
                .headers
                .authentication_results,
            None
        );
    }

    #[test]
    fn a_topmost_header_that_is_not_utf8_means_no_results() {
        let mut raw = b"Authentication-Results: mx.google.com; dmarc=pass \xff\r\n".to_vec();
        raw.extend_from_slice(with_results(&[GMAIL_PASS]).as_bytes());
        assert_eq!(
            parse(&raw, Receiver::Gmail).headers.authentication_results,
            None
        );
    }
}
