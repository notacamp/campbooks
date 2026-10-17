//! A message's own words as plain text, for search: HTML reduced to text,
//! quoted history removed, bounded. The HTML is read with the tokenizer
//! alone (no tree), within [`MAX_TEXT_TIME`].

use std::{
    cell::{Cell, RefCell},
    sync::LazyLock,
    time::{Duration, Instant},
};

use html5ever::{
    tendril::StrTendril,
    tokenizer::{
        BufferQueue, TagKind, Token, TokenSink, TokenSinkResult, Tokenizer, TokenizerOpts,
        states::RawKind,
    },
};
use regex::Regex;

/// The most text handed on per message, in bytes.
pub const MAX_BYTES: usize = 32 * 1024;

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

/// Whether `<blockquote>` text is kept by [`text_of`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quotes {
    /// Keep it: the reader shows the whole message.
    Keep,
    /// Drop it: search wants the message's own words.
    Drop,
}

/// The text of an HTML body, without quoted `<blockquote>`s: see
/// [`text_of`].
pub fn from_html(html: &str) -> String {
    text_of(html, Quotes::Drop)
}

/// The most time reading the text of one body may take. Search wants the
/// words: HTML whose tokenizing takes longer than this (only crafted HTML,
/// such as a single tag with tens of thousands of attributes) gives the
/// words read so far.
pub const MAX_TEXT_TIME: Duration = Duration::from_millis(500);
/// Bytes of HTML fed to the tokenizer between two reads of the clock.
const CHUNK: usize = 1024;

/// The text of an HTML body, read with html5ever's tokenizer alone (no
/// tree, so however the tags nest; stopped at [`MAX_TEXT_TIME`]): no tags, no
/// scripts, styles, head or title, entities decoded, a line break at each
/// `br` and at the end of each block, and no `<blockquote>`s when `quotes`
/// is [`Quotes::Drop`].
pub fn text_of(html: &str, quotes: Quotes) -> String {
    let deadline = Instant::now() + MAX_TEXT_TIME;
    let tokenizer = Tokenizer::new(Reader::new(quotes), TokenizerOpts::default());
    let queue = BufferQueue::default();
    let mut rest = html;
    while !rest.is_empty() && Instant::now() <= deadline {
        let mut end = rest.len().min(CHUNK);
        while !rest.is_char_boundary(end) {
            end -= 1;
        }
        let (chunk, tail) = rest.split_at(end);
        queue.push_back(StrTendril::from_slice(chunk));
        let _ = tokenizer.feed(&queue);
        rest = tail;
    }
    tokenizer.end();
    tokenizer.sink.text.take()
}

/// Collects the characters outside the elements whose content is not text
/// to read.
struct Reader {
    quotes: Quotes,
    text: RefCell<String>,
    /// Inside an element whose raw text is not words to read (`script`,
    /// `style`, `title`, and the markup a `noscript`, `iframe`, `noembed` or
    /// `noframes` holds); raw text cannot nest, so its end tag ends it.
    skipping: Cell<bool>,
    in_head: Cell<bool>,
    quote_depth: Cell<usize>,
}

impl Reader {
    fn new(quotes: Quotes) -> Self {
        Self {
            quotes,
            text: RefCell::new(String::new()),
            skipping: Cell::new(false),
            in_head: Cell::new(false),
            quote_depth: Cell::new(0),
        }
    }
}

impl TokenSink for Reader {
    type Handle = ();

    fn process_token(&self, token: Token, _line: u64) -> TokenSinkResult<()> {
        match token {
            Token::TagToken(tag) => {
                let name = &*tag.name;
                if tag.kind == TagKind::EndTag {
                    match name {
                        "script" | "style" | "title" | "noscript" | "iframe" | "noembed"
                        | "noframes" => self.skipping.set(false),
                        "head" => self.in_head.set(false),
                        "blockquote" => {
                            self.quote_depth
                                .set(self.quote_depth.get().saturating_sub(1));
                        }
                        "p" | "div" | "li" | "tr" | "td" | "h1" | "h2" | "h3" | "h4" | "h5"
                        | "h6" | "table" | "ul" | "ol" => self.text.borrow_mut().push('\n'),
                        _ => {}
                    }
                    return TokenSinkResult::Continue;
                }
                match name {
                    "head" => self.in_head.set(true),
                    "body" => self.in_head.set(false),
                    "blockquote" => self.quote_depth.set(self.quote_depth.get() + 1),
                    "br" => self.text.borrow_mut().push('\n'),
                    "script" | "style" | "title" | "noscript" | "iframe" | "noembed"
                    | "noframes" => self.skipping.set(true),
                    _ => {}
                }
                // Raw-text switching as the tree builder does it
                // (`styles::scan` does the same).
                match name {
                    "style" | "xmp" | "iframe" | "noembed" | "noframes" | "noscript" => {
                        TokenSinkResult::RawData(RawKind::Rawtext)
                    }
                    "title" | "textarea" => TokenSinkResult::RawData(RawKind::Rcdata),
                    "script" => TokenSinkResult::RawData(RawKind::ScriptData),
                    "plaintext" => TokenSinkResult::Plaintext,
                    _ => TokenSinkResult::Continue,
                }
            }
            Token::CharacterTokens(text) => {
                let hidden = self.skipping.get()
                    || self.in_head.get()
                    || (self.quotes == Quotes::Drop && self.quote_depth.get() > 0);
                if !hidden {
                    let mut out = self.text.borrow_mut();
                    out.extend(text.chars().map(|c| if c == '\u{a0}' { ' ' } else { c }));
                }
                TokenSinkResult::Continue
            }
            _ => TokenSinkResult::Continue,
        }
    }
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

#[cfg(test)]
mod tests {
    use std::fmt::Write as _;

    use super::*;

    /// A string built by `add` called for 0..count.
    fn repeated_with(count: usize, add: impl Fn(&mut String, usize)) -> String {
        let mut text = String::new();
        for i in 0..count {
            add(&mut text, i);
        }
        text
    }

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

    #[test]
    fn hostile_nesting_is_read_in_linear_time() {
        let html = format!("<p>Word</p>{}", "<div></span>".repeat(170_000));
        assert!(from_html(&html).contains("Word"));
    }

    #[test]
    fn markup_held_as_raw_text_is_not_words() {
        let html = "<p>Hi</p><noscript><img src=\"https://t.example/p.gif\"></noscript>\
                    <iframe><b>frame</b></iframe><xmp>shown</xmp>";
        let text = from_html(html);
        assert!(text.contains("Hi") && text.contains("shown"), "{text}");
        assert!(!text.contains('<') && !text.contains("frame"), "{text}");
    }

    #[test]
    fn the_reader_keeps_quotes_that_search_drops() {
        let html = "<div>New</div><blockquote>Old</blockquote>";
        assert!(text_of(html, Quotes::Keep).contains("Old"));
        assert!(!from_html(html).contains("Old"));
        assert!(from_html(html).contains("New"));
    }

    #[test]
    fn a_tag_with_countless_attributes_returns() {
        let many = repeated_with(60_000, |s, i| {
            let _ = write!(s, "a{i} ");
        });
        let text = text_of(&format!("<p>x</p><p {many}>y</p>"), Quotes::Keep);
        assert!(text.contains('x'), "{text}");
    }
}
