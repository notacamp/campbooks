//! Untrusted text made safe to display: what a sender wrote (a quote, a file
//! name) can carry characters that reorder or hide what the reader sees.
//! And a message's own words without the history it quotes: text hygiene,
//! never a decision about what the message means.

use std::sync::LazyLock;

use regex::Regex;

/// Where quoted history starts: an attribution line ("On Tue, Ana wrote:",
/// possibly wrapped onto a second line) in the four languages, or an
/// "original message" separator.
static QUOTE_START: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"(?mi)^[ \t]*(?:",
        r"(?:On|Em|El|Le)\b[^\n]{0,200}\n?[^\n]{0,100}\b(?:wrote|escreveu|escribió|a écrit)[ \t]*:[ \t]*$",
        r"|-{2,}[ \t]*(?:Original Message|Mensagem original|Mensaje original|Message d'origine)[ \t]*-{2,}",
        r"|(?:From|De)[ \t]*:[^\n]*\n[ \t]*(?:Sent|Date|Enviado|Enviada|Data|Fecha|Envoyé)[ \t]*:",
        r")"
    ))
    .unwrap_or_else(|e| unreachable!("static regex compiles: {e}"))
});

/// Three or more line breaks (with blanks between them).
static BLANK_LINES: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\n[ \t]*(?:\n[ \t]*){2,}")
        .unwrap_or_else(|e| unreachable!("static regex compiles: {e}"))
});

/// `text` without quoted history: the attribution line and everything
/// after it, and lines starting with `>`; trailing blanks trimmed and blank
/// runs collapsed. Shared by every module that hands a message's own words
/// on (search, Scout's replies, Scout's chat).
pub fn without_quoted_history(text: &str) -> String {
    let text = text.replace("\r\n", "\n");
    let own = QUOTE_START
        .find(&text)
        .map_or(text.as_str(), |quote| &text[..quote.start()]);
    let kept: Vec<&str> = own
        .lines()
        .filter(|line| !line.trim_start().starts_with('>'))
        .map(str::trim_end)
        .collect();
    let joined = kept.join("\n");
    BLANK_LINES.replace_all(joined.trim(), "\n\n").into_owned()
}

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

    #[test]
    fn quoted_history_goes_in_four_languages() {
        for attribution in [
            "On Tue, 7 Oct 2026 at 09:05, Ana Lopes <ana@example.com> wrote:",
            "Em ter., 7/10/2026, Ana <ana@example.com> escreveu:",
            "El mar, 7 oct 2026, Ana <ana@example.com> escribió:",
            "Le mar. 7 oct. 2026, Ana <ana@example.com> a écrit :",
            "-----Original Message-----",
        ] {
            let text = format!("Will do.\r\n\n\n\n{attribution}\n> The old text\nMore old text");
            assert_eq!(without_quoted_history(&text), "Will do.", "{attribution}");
        }
        assert_eq!(
            without_quoted_history("First\n> quoted\n\n\n\nLast   "),
            "First\n\nLast"
        );
    }
}
