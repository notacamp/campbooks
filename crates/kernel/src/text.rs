//! Untrusted text made safe to display: what a sender wrote (a quote, a file
//! name) can carry characters that reorder or hide what the reader sees.

/// Characters that reorder text (bidirectional overrides, embeddings,
/// isolates and marks): `invoice\u{202E}fdp.exe` would read as
/// `invoiceexe.pdf`.
pub const fn is_bidi_control(c: char) -> bool {
    matches!(c, '\u{200E}' | '\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}')
}

/// Characters that take no room and hide differences (zero-width space,
/// joiners, word joiner, byte-order mark).
pub const fn is_invisible(c: char) -> bool {
    matches!(c, '\u{200B}'..='\u{200D}' | '\u{2060}' | '\u{FEFF}')
}

/// Whether `c` may stay in text shown to a member: no control characters
/// (C0, DEL, C1), no bidirectional controls and no invisible characters.
pub fn is_displayable(c: char) -> bool {
    !c.is_control() && !is_bidi_control(c) && !is_invisible(c)
}

/// `text` without the characters [`is_displayable`] refuses.
pub fn displayable(text: &str) -> String {
    text.chars().filter(|c| is_displayable(*c)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reordering_hidden_and_control_characters_go() {
        assert_eq!(
            displayable("IBAN PT50\u{202E}0002\u{200B} 0123\u{0007}\u{0085}"),
            "IBAN PT500002 0123"
        );
        assert_eq!(displayable("Olá, João ✓"), "Olá, João ✓");
    }
}
