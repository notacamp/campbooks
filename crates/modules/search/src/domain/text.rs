//! Preparing text for the index.

/// The characters highlights are delimited with (Unicode private use).
pub const HIT_START: char = '\u{e000}';
/// See [`HIT_START`].
pub const HIT_END: char = '\u{e001}';

/// `text` without the highlight delimiters (so stored mail cannot forge a
/// highlight) and without other control characters, line breaks kept.
pub fn clean(text: &str) -> String {
    text.chars()
        .filter(|&c| c != HIT_START && c != HIT_END)
        .map(|c| {
            if c.is_control() && c != '\n' && c != '\t' {
                ' '
            } else {
                c
            }
        })
        .collect()
}

/// `text` split into words at anything that is not a letter or a digit, so
/// `ana.lopes@example.com` and `q4-plan.pdf` are found by their parts.
pub fn words(text: &str) -> String {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delimiters_and_controls_go() {
        assert_eq!(clean("a\u{e000}b\u{e001}c\u{7}d\ne"), "abc d\ne");
    }

    #[test]
    fn addresses_and_file_names_split_into_words() {
        assert_eq!(
            words("Ana Lopes <ana.lopes@example.com> q4-plan.pdf"),
            "Ana Lopes ana lopes example com q4 plan pdf"
        );
    }
}
