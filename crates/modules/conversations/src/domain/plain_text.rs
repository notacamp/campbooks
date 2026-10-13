//! A message's own words as plain text, for search: HTML reduced to text,
//! quoted history removed, bounded.

use std::{
    collections::{HashMap, HashSet},
    sync::LazyLock,
};

use ammonia::Builder;
use regex::Regex;

/// The most text handed on per message, in bytes.
pub const MAX_BYTES: usize = 32 * 1024;

/// Block-level closing tags (and `<br>`): a line break goes before each, so
/// words on both sides don't run together once the tags are gone.
static BLOCK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)<(br|/p|/div|/li|/tr|/td|/h[1-6]|/table|/ul|/ol)\b")
        .unwrap_or_else(|e| unreachable!("static regex compiles: {e}"))
});

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

/// The text of an HTML body: no tags, no scripts or styles, no quoted
/// `<blockquote>`s, entities decoded.
pub fn from_html(html: &str) -> String {
    let marked = BLOCK.replace_all(html, "\n<$1");
    let removed: HashSet<&str> = ["script", "style", "head", "title", "blockquote"].into();
    let text = Builder::empty()
        .tag_attributes(HashMap::new())
        .clean_content_tags(removed)
        .clean(&marked)
        .to_string();
    unescape(&text)
}

/// `text` without quoted history (the attribution line and everything after
/// it, and lines starting with `>`), blank runs collapsed, at most
/// [`MAX_BYTES`].
pub fn own_words(text: &str) -> String {
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
    let collapsed = BLANK_LINES.replace_all(joined.trim(), "\n\n");
    truncate(&collapsed, MAX_BYTES).to_owned()
}

/// `text` cut to at most `max` bytes, at a character boundary.
fn truncate(text: &str, max: usize) -> &str {
    if text.len() <= max {
        return text;
    }
    let mut end = max;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

/// Decode the entities the HTML serialiser writes.
fn unescape(text: &str) -> String {
    text.replace("&nbsp;", " ")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&amp;", "&")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_becomes_words_without_markup() {
        let html = "<html><head><title>T</title><style>p{color:red}</style></head><body>\
                    <p>Hello&nbsp;Sam,</p><p>See the <b>Q4</b> plan &amp; budget.</p>\
                    <script>alert(1)</script><div>Thanks<br>Maya</div></body></html>";
        let text = from_html(html);
        assert!(text.contains("Hello Sam,"), "{text}");
        assert!(text.contains("See the Q4 plan & budget."), "{text}");
        assert!(text.contains("Thanks\nMaya"), "{text}");
        assert!(!text.contains("alert") && !text.contains("color") && !text.contains('<'));
    }

    #[test]
    fn quoted_blockquotes_are_dropped() {
        let html = "<div>Sounds good.</div><div class=\"gmail_quote\">On Tue, Ana wrote:\
                    <blockquote>The old text</blockquote></div>";
        let text = own_words(&from_html(html));
        assert_eq!(text, "Sounds good.");
    }

    #[test]
    fn attribution_lines_end_the_text_in_four_languages() {
        for attribution in [
            "On Tue, 7 Oct 2026 at 09:05, Ana Lopes <ana@example.com> wrote:",
            "On Tue, 7 Oct 2026 at 09:05, Ana Lopes <ana@example.com>\nwrote:",
            "Em ter., 7/10/2026, Ana <ana@example.com> escreveu:",
            "El mar, 7 oct 2026, Ana <ana@example.com> escribió:",
            "Le mar. 7 oct. 2026, Ana <ana@example.com> a écrit :",
            "-----Original Message-----",
            "From: Ana Lopes\nSent: Tuesday",
            "De: Ana Lopes\nEnviado: terça-feira",
        ] {
            let text = format!("Will do.\n\n{attribution}\n> The old text\nMore old text");
            assert_eq!(own_words(&text), "Will do.", "{attribution}");
        }
    }

    #[test]
    fn quoted_lines_and_blank_runs_go() {
        let text = "First line\n> quoted\n\n\n\nLast line   \r\n";
        assert_eq!(own_words(text), "First line\n\nLast line");
    }

    #[test]
    fn text_is_bounded_at_a_character_boundary() {
        let text = "é".repeat(MAX_BYTES);
        let own = own_words(&text);
        assert!(own.len() <= MAX_BYTES && own.len() > MAX_BYTES - 2);
        assert!(own.chars().all(|c| c == 'é'));
    }
}
