//! The person a reply answers, as far as deterministic code knows them: an
//! address plain enough to look up, and the characters kept from words a
//! member or a model wrote (campbooks-cloud#271, #272).

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

/// Whether a character of written text is kept: no control characters but
/// line breaks and tabs, and no invisible direction or zero-width marks
/// that could make the text read differently from what was shown.
pub fn kept_char(c: char) -> bool {
    let invisible = matches!(
        c,
        '\u{200B}'..='\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}' | '\u{FEFF}'
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
    }
}
