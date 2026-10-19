//! The person a reply answers, as far as deterministic code knows them: an
//! address plain enough to look up, whether a message from them is
//! distrusted, and the characters kept from words a member or a model wrote
//! (campbooks-cloud#271, #272).

use campbooks_ai::decide::signals::facts::{dmarc_refused, sender_checks_failed};
use serde_json::Value;

/// Whether `address` is plain enough to look up: one `@` with something on
/// both sides, and no quote, backslash, whitespace or control character.
pub fn plain_address(address: &str) -> bool {
    let mut parts = address.split('@');
    let (Some(local), Some(domain), None) = (parts.next(), parts.next(), parts.next()) else {
        return false;
    };
    !local.is_empty()
        && !domain.is_empty()
        && !address
            .chars()
            .any(|c| c == '"' || c == '\\' || c.is_whitespace() || c.is_control())
}

/// Whether a message earns no trust about its sender. Facts only, as
/// everywhere else (ADR 0018): spam, a `suspicious` answer or open Review
/// need, and, for mail received, the receiving provider's results saying the
/// sender failed its checks or is impersonated. Mail with no results is not
/// taken for forged.
pub fn distrusted(
    direction: &str,
    sender: &str,
    in_spam: bool,
    headers: &Value,
    suspicious: bool,
) -> bool {
    if in_spam || suspicious {
        return true;
    }
    let results = headers
        .get("authentication_results")
        .and_then(Value::as_str);
    direction == "inbound" && (sender_checks_failed(sender, results) || dmarc_refused(results))
}

/// Whether a character of written text is kept: no control characters but
/// line breaks and tabs, and no invisible marks (direction marks and
/// overrides, zero-width and joining characters, soft hyphens, invisible
/// operators, deprecated format characters) that could make the text read
/// differently from what was shown.
pub fn kept_char(c: char) -> bool {
    let invisible = matches!(
        c,
        '\u{00AD}'
            | '\u{061C}'
            | '\u{180E}'
            | '\u{200B}'..='\u{200F}'
            | '\u{202A}'..='\u{202E}'
            | '\u{2060}'..='\u{2064}'
            | '\u{2066}'..='\u{206F}'
            | '\u{FEFF}'
    );
    (!c.is_control() || c == '\n' || c == '\t') && !invisible
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_plain_address_is_looked_up() {
        assert!(plain_address("marta@clientes.example"));
        for forged in [
            "marta",
            "@clientes.example",
            "marta@",
            "a@b@c.example",
            "x\"@y.example",
            "x\\y@z.example",
            "marta @clientes.example",
            "marta@clientes.example\n",
        ] {
            assert!(!plain_address(forged), "{forged:?}");
        }
    }

    #[test]
    fn invisible_marks_and_controls_are_dropped() {
        let kept: String = "Formal\u{202E}, in\u{200B} Portuguese.\u{0007}\n\tOk"
            .chars()
            .filter(|c| kept_char(*c))
            .collect();
        assert_eq!(kept, "Formal, in Portuguese.\n\tOk");
        for invisible in [
            '\u{00AD}', '\u{061C}', '\u{180E}', '\u{2060}', '\u{2061}', '\u{2062}', '\u{2063}',
            '\u{2064}', '\u{206A}', '\u{206B}', '\u{206C}', '\u{206D}', '\u{206E}', '\u{206F}',
        ] {
            assert!(!kept_char(invisible), "U+{:04X}", u32::from(invisible));
        }
        // Visible text around them is kept, in any script.
        for visible in ['a', 'é', 'ç', 'ã', 'ß', 'ي', 'ᠠ', '€', ' '] {
            assert!(kept_char(visible), "{visible:?}");
        }
    }

    #[test]
    fn only_facts_distrust_a_sender() {
        let failed = serde_json::json!({
            "authentication_results":
                "mx.example; dkim=fail header.i=@clientes.example; dmarc=fail header.from=clientes.example"
        });
        let none = serde_json::json!({});
        let sender = "marta@clientes.example";
        assert!(distrusted("inbound", sender, false, &failed, false));
        assert!(distrusted("inbound", sender, true, &none, false));
        assert!(distrusted("inbound", sender, false, &none, true));
        // No results: not taken for forged.
        assert!(!distrusted("inbound", sender, false, &none, false));
        // The member's own mail is never failing a sender's checks.
        assert!(!distrusted("outbound", sender, false, &failed, false));
    }
}
