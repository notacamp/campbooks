//! The member's own words, made safe to use: what they typed may carry
//! characters that hide or reorder text, or be something pasted from mail
//! rather than their own sentence.

use campbooks_kernel::text::displayable;

/// Longest instruction, in characters.
pub const MAX_CHARS: usize = 300;

/// Why an instruction is refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// Nothing left once cleaned.
    Empty,
    /// Longer than [`MAX_CHARS`].
    TooLong,
    /// Holds a link.
    Url,
    /// Looks pasted from an email.
    PastedMail,
}

impl Refusal {
    /// The problem's code.
    pub const fn code(self) -> &'static str {
        match self {
            Self::Empty => "instruction_empty",
            Self::TooLong => "instruction_too_long",
            Self::Url => "instruction_has_link",
            Self::PastedMail => "instruction_pasted_mail",
        }
    }
}

/// Header names that start a line of a pasted email, in the four languages.
const HEADER_LINES: [&str; 16] = [
    "from:", "to:", "cc:", "subject:", "date:", "sent:", "de:", "para:", "assunto:", "asunto:",
    "enviado:", "data:", "fecha:", "objet:", "à:", "envoyé:",
];

/// `raw` cleaned (displayable characters only, whitespace collapsed,
/// trimmed), or why it is refused.
pub fn own_words(raw: &str) -> Result<String, Refusal> {
    if looks_pasted(raw) {
        return Err(Refusal::PastedMail);
    }
    let cleaned = displayable(&raw.replace(['\n', '\r', '\t'], " "))
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if cleaned.is_empty() {
        return Err(Refusal::Empty);
    }
    if cleaned.chars().count() > MAX_CHARS {
        return Err(Refusal::TooLong);
    }
    let lower = cleaned.to_lowercase();
    if ["http://", "https://", "www.", "://"]
        .iter()
        .any(|marker| lower.contains(marker))
    {
        return Err(Refusal::Url);
    }
    Ok(cleaned)
}

/// Several lines, a header line, a quote marker or a run of addresses.
fn looks_pasted(raw: &str) -> bool {
    let lines: Vec<&str> = raw
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    let header = lines.iter().any(|line| {
        let lower = line.to_lowercase().replace(" :", ":");
        line.starts_with('>') || HEADER_LINES.iter().any(|h| lower.starts_with(h))
    });
    let addresses = raw.matches('@').count();
    lines.len() > 2 || header || addresses > 2
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hidden_characters_go_and_spaces_collapse() {
        assert_eq!(
            own_words("  anything from my\u{202E} accountant\u{200B}   always\tmatters ").unwrap(),
            "anything from my accountant always matters"
        );
        assert_eq!(
            own_words("Olá, sempre o João").unwrap(),
            "Olá, sempre o João"
        );
    }

    #[test]
    fn refusals() {
        assert_eq!(own_words(" \u{200B} "), Err(Refusal::Empty));
        assert_eq!(own_words(&"a".repeat(301)), Err(Refusal::TooLong));
        assert!(own_words(&"a".repeat(300)).is_ok());
        assert_eq!(
            own_words("archive https://example.com/x"),
            Err(Refusal::Url)
        );
        assert_eq!(own_words("see www.example.com"), Err(Refusal::Url));
        assert_eq!(
            own_words("From: Ana <ana@example.com>\nSubject: Invoice"),
            Err(Refusal::PastedMail)
        );
        assert_eq!(own_words("> forwarded text"), Err(Refusal::PastedMail));
        assert_eq!(
            own_words("Objet : facture de septembre"),
            Err(Refusal::PastedMail)
        );
        assert_eq!(
            own_words("a@example.com b@example.com c@example.com"),
            Err(Refusal::PastedMail)
        );
        assert_eq!(own_words("one\ntwo\nthree"), Err(Refusal::PastedMail));
        assert!(own_words("mail from rita@silva-contas.example\nalways matters").is_ok());
    }
}
