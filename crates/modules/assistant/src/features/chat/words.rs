//! Text going in and out of a chat, made safe: no control, bidirectional or
//! invisible characters, line breaks kept.

use campbooks_kernel::text::is_displayable;

/// Longest question, in characters.
pub const MAX_QUESTION_CHARS: usize = 2_000;
/// Longest answer kept, in characters.
pub const MAX_ANSWER_CHARS: usize = 6_000;

/// Why a question is refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// Nothing left once cleaned.
    Empty,
    /// Longer than [`MAX_QUESTION_CHARS`].
    TooLong,
}

impl Refusal {
    /// The problem's code.
    pub const fn code(self) -> &'static str {
        match self {
            Self::Empty => "question_empty",
            Self::TooLong => "question_too_long",
        }
    }
}

/// `text` with only displayable characters and line breaks (tabs become
/// spaces, `\r\n` a line break), trimmed.
pub fn clean(text: &str) -> String {
    text.replace("\r\n", "\n")
        .chars()
        .map(|c| if c == '\t' { ' ' } else { c })
        .filter(|c| *c == '\n' || is_displayable(*c))
        .collect::<String>()
        .trim()
        .to_owned()
}

/// A piece of streamed text with only displayable characters and line
/// breaks, not trimmed (pieces join up).
pub fn kept(delta: &str) -> String {
    delta
        .chars()
        .map(|c| if c == '\t' { ' ' } else { c })
        .filter(|c| *c == '\n' || is_displayable(*c))
        .collect()
}

/// The member's question, cleaned, or why it is refused.
pub fn question(raw: &str) -> Result<String, Refusal> {
    let cleaned = clean(raw);
    if cleaned.is_empty() {
        return Err(Refusal::Empty);
    }
    if cleaned.chars().count() > MAX_QUESTION_CHARS {
        return Err(Refusal::TooLong);
    }
    Ok(cleaned)
}

/// Scout's answer as it is kept: cleaned and bounded.
pub fn answer(raw: &str) -> String {
    let cleaned = clean(raw);
    if cleaned.chars().count() <= MAX_ANSWER_CHARS {
        return cleaned;
    }
    cleaned.chars().take(MAX_ANSWER_CHARS).collect()
}

/// `text` cut to at most `max` bytes, at a character boundary.
pub fn cut_bytes(text: &str, max: usize) -> &str {
    if text.len() <= max {
        return text;
    }
    let mut end = max;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hidden_and_reordering_characters_go_line_breaks_stay() {
        assert_eq!(
            question("  Why is this\u{202E} here?\r\nAnd\u{200B} that?\u{0007}  ").unwrap(),
            "Why is this here?\nAnd that?"
        );
    }

    #[test]
    fn empty_and_long_questions_are_refused() {
        assert_eq!(question(" \u{200B}\n\t"), Err(Refusal::Empty));
        assert_eq!(
            question(&"é".repeat(MAX_QUESTION_CHARS + 1)),
            Err(Refusal::TooLong)
        );
        assert!(question(&"é".repeat(MAX_QUESTION_CHARS)).is_ok());
    }

    #[test]
    fn answers_are_bounded_and_cuts_keep_characters_whole() {
        assert_eq!(
            answer(&"a".repeat(MAX_ANSWER_CHARS + 10)).len(),
            MAX_ANSWER_CHARS
        );
        assert_eq!(cut_bytes("aé", 2), "a");
    }
}
