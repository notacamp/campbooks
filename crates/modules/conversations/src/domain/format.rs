//! The basic formatting a member may use in a message, rendered to the HTML
//! alternative of what they send.
//!
//! Safe by construction: every character is escaped first, and only these
//! marks become markup: `**bold**`, `*italic*`, `~~strikethrough~~`, lines
//! starting `- ` or `* ` (a bulleted list), `1. ` (a numbered list), `> ` (a
//! quote, nested by repeating it), bare `http://` / `https://` links and
//! `[words](url)` links to http, https or mailto addresses. A `\` makes the
//! next mark character plain text. Blank lines separate paragraphs; single
//! line breaks are kept. There is no way to write raw HTML.
//!
//! Two sets of marks ([`Marks`]): the member's own text gets all of them; the
//! quoted history comes from someone else's mail, so it gets only the bare
//! links, emphasis, lists and quotes, never words that hide where a link goes.

use std::fmt::Write as _;

/// Which marks apply to a piece of text.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Marks {
    /// What the member wrote: every mark, and `\` escapes.
    Own,
    /// A quoted history: bare links, emphasis, lists and quotes only.
    Quoted,
}

/// `text` (the member's own) as HTML.
pub fn to_html(text: &str, marks: Marks) -> String {
    to_html_parts(&[(text, marks)])
}

/// Several pieces of text, each with its own marks, as the HTML of one message.
pub fn to_html_parts(parts: &[(&str, Marks)]) -> String {
    let mut html = String::from("<div>");
    for (text, marks) in parts {
        let lines: Vec<&str> = text.lines().collect();
        blocks(&lines, *marks, &mut html);
    }
    html.push_str("</div>");
    html
}

/// The HTML and plain-text parts of a message: the member's `body` with all
/// the marks, then the `quoted` history with the quoted ones only.
pub fn message(body: &str, quoted: &str) -> (String, String) {
    if quoted.trim().is_empty() {
        return (to_html(body, Marks::Own), to_plain(body));
    }
    let body = body.trim_end();
    (
        to_html_parts(&[(body, Marks::Own), (quoted, Marks::Quoted)]),
        format!("{}\n\n{quoted}", to_plain(body)),
    )
}

/// `text` with every `[` and `]` made plain (`\\[`, `\\]`), so that no
/// `[words](url)` in it becomes a link whose words hide where it goes. For
/// text the member did not type: the assistant's, which may follow
/// instructions hidden in someone else's mail. Escapes already there stay.
pub fn literal_links(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                out.push(c);
                if let Some(next) = chars.next() {
                    out.push(next);
                }
            }
            '[' | ']' => {
                out.push('\\');
                out.push(c);
            }
            c => out.push(c),
        }
    }
    out
}

/// The member's own `text` for the plain-text part: a link with words is
/// `words <url>`, escapes are dropped, the other marks stay as typed.
pub fn to_plain(text: &str) -> String {
    let mut out = String::new();
    for (n, line) in text.split('\n').enumerate() {
        if n > 0 {
            out.push('\n');
        }
        let (prefix, content) = split_prefix(line);
        out.push_str(prefix);
        plain_line(content, true, &mut out);
    }
    out
}

/// The quote and list prefixes of a line, and what follows them.
fn split_prefix(line: &str) -> (&str, &str) {
    let mut at = 0;
    loop {
        let rest = &line[at..];
        if let Some(r) = rest.strip_prefix("> ") {
            at = line.len() - r.len();
        } else if rest.starts_with('>') {
            at += 1;
        } else {
            break;
        }
    }
    let rest = &line[at..];
    let item = bullet(rest).or_else(|| numbered(rest));
    if let Some(content) = item {
        at = line.len() - content.len();
    }
    line.split_at(at)
}

fn plain_line(line: &str, first: bool, out: &mut String) {
    let mut rest = line;
    let mut at_start = first;
    while !rest.is_empty() {
        if at_start && let Some((lit, after)) = leading_escape(rest) {
            out.push_str(&lit);
            rest = after;
            at_start = false;
            continue;
        }
        at_start = false;
        if let Some((c, after)) = escaped(rest) {
            out.push(c);
            rest = after;
            continue;
        }
        if let Some((words, url, after)) = worded_link(rest) {
            plain_line(words, false, out);
            let _ = write!(out, " <{url}>");
            rest = after;
            continue;
        }
        let mut chars = rest.chars();
        if let Some(c) = chars.next() {
            out.push(c);
        }
        rest = chars.as_str();
    }
}

/// A `\` and the mark character it makes plain, anywhere in a line.
fn escaped(text: &str) -> Option<(char, &str)> {
    let after = text.strip_prefix('\\')?;
    let c = after.chars().next()?;
    matches!(c, '*' | '~' | '[' | ']' | '\\').then(|| (c, &after[c.len_utf8()..]))
}

/// An escaped leading `-`, `>` or `1.`, at the start of a line.
fn leading_escape(text: &str) -> Option<(String, &str)> {
    let digits = text.bytes().take_while(u8::is_ascii_digit).count();
    if digits > 0 {
        let r = text[digits..].strip_prefix("\\.")?;
        return (digits <= 9).then(|| (format!("{}.", &text[..digits]), r));
    }
    let after = text.strip_prefix('\\')?;
    if let Some(r) = after.strip_prefix('-') {
        return Some(("-".into(), r));
    }
    after.strip_prefix('>').map(|r| (">".into(), r))
}

/// `[words](url)` at the start of `text`: the words, the address and the rest.
fn worded_link(text: &str) -> Option<(&str, &str, &str)> {
    let inner = text.strip_prefix('[')?;
    let mut end = None;
    let mut chars = inner.char_indices();
    while let Some((i, c)) = chars.next() {
        match c {
            '\\' => {
                chars.next();
            }
            ']' => {
                end = Some(i);
                break;
            }
            _ => {}
        }
    }
    let end = end?;
    let words = &inner[..end];
    let after = inner[end + 1..].strip_prefix('(')?;
    let close = after.find(')')?;
    let url = &after[..close];
    (!words.is_empty() && safe_url(url)).then(|| (words, url, &after[close + 1..]))
}

/// An http, https or mailto address with something after the scheme and no
/// whitespace, `<`, `>` or `"`.
fn safe_url(url: &str) -> bool {
    let scheme = ["https://", "http://", "mailto:"]
        .iter()
        .find(|s| url.starts_with(**s))
        .map(|s| s.len());
    scheme.is_some_and(|n| url.len() > n)
        && !url.contains(|c: char| c.is_whitespace() || matches!(c, '<' | '>' | '"'))
}

/// The lines starting with `> `, without it.
fn unquote<'a>(lines: &[&'a str]) -> Vec<&'a str> {
    lines
        .iter()
        .map(|line| {
            line.strip_prefix("> ")
                .or_else(|| line.strip_prefix('>'))
                .unwrap_or(line)
        })
        .collect()
}

fn is_quote(line: &str) -> bool {
    line.starts_with('>')
}

fn bullet(line: &str) -> Option<&str> {
    line.strip_prefix("- ").or_else(|| line.strip_prefix("* "))
}

fn numbered(line: &str) -> Option<&str> {
    let digits = line.bytes().take_while(u8::is_ascii_digit).count();
    if digits == 0 || digits > 9 {
        return None;
    }
    line[digits..].strip_prefix(". ")
}

fn blocks(lines: &[&str], marks: Marks, html: &mut String) {
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        if line.trim().is_empty() {
            i += 1;
        } else if is_quote(line) {
            let end = run(lines, i, is_quote);
            html.push_str("<blockquote>");
            blocks(&unquote(&lines[i..end]), marks, html);
            html.push_str("</blockquote>");
            i = end;
        } else if bullet(line).is_some() {
            let end = run(lines, i, |l| bullet(l).is_some());
            list(
                "ul",
                lines[i..end].iter().filter_map(|l| bullet(l)),
                marks,
                html,
            );
            i = end;
        } else if numbered(line).is_some() {
            let end = run(lines, i, |l| numbered(l).is_some());
            list(
                "ol",
                lines[i..end].iter().filter_map(|l| numbered(l)),
                marks,
                html,
            );
            i = end;
        } else {
            let end = run(lines, i, |l| {
                !l.trim().is_empty() && !is_quote(l) && bullet(l).is_none() && numbered(l).is_none()
            });
            html.push_str("<p>");
            for (n, line) in lines[i..end].iter().enumerate() {
                if n > 0 {
                    html.push_str("<br>");
                }
                inline(line, marks, true, true, html);
            }
            html.push_str("</p>");
            i = end;
        }
    }
}

/// The end of the run of lines from `start` that satisfy `keep`.
fn run(lines: &[&str], start: usize, keep: impl Fn(&str) -> bool) -> usize {
    let mut end = start;
    while end < lines.len() && keep(lines[end]) {
        end += 1;
    }
    end
}

fn list<'a>(tag: &str, items: impl Iterator<Item = &'a str>, marks: Marks, html: &mut String) {
    let _ = write!(html, "<{tag}>");
    for item in items {
        html.push_str("<li>");
        inline(item, marks, true, true, html);
        html.push_str("</li>");
    }
    let _ = write!(html, "</{tag}>");
}

#[derive(Clone, Copy, PartialEq)]
enum Span {
    Bold,
    Italic,
    Strike,
}

impl Span {
    fn tag(self) -> &'static str {
        match self {
            Span::Bold => "strong",
            Span::Italic => "em",
            Span::Strike => "s",
        }
    }
}

/// Opens or closes `span`, keeping the tags properly nested.
fn toggle(open: &mut Vec<Span>, span: Span, html: &mut String) {
    let Some(at) = open.iter().position(|s| *s == span) else {
        open.push(span);
        let _ = write!(html, "<{}>", span.tag());
        return;
    };
    let above: Vec<Span> = open.split_off(at);
    for s in above.iter().rev() {
        let _ = write!(html, "</{}>", s.tag());
    }
    for s in &above[1..] {
        open.push(*s);
        let _ = write!(html, "<{}>", s.tag());
    }
}

/// One line: emphasis and links, everything else escaped. `first` is whether
/// the line starts here (for a leading escape); `links` whether a
/// `[words](url)` may become a link (not inside another one).
fn inline(line: &str, marks: Marks, first: bool, links: bool, html: &mut String) {
    let own = marks == Marks::Own;
    let mut rest = line;
    let mut at_start = first;
    let mut open: Vec<Span> = Vec::new();
    while !rest.is_empty() {
        let start = at_start;
        at_start = false;
        if own {
            if start && let Some((lit, after)) = leading_escape(rest) {
                escape(&lit, html);
                rest = after;
                continue;
            }
            if let Some((c, after)) = escaped(rest) {
                escape_char(c, html);
                rest = after;
                continue;
            }
            if let Some(after) = rest.strip_prefix("~~")
                && (open.contains(&Span::Strike) || has_unescaped(after, "~~"))
            {
                toggle(&mut open, Span::Strike, html);
                rest = after;
                continue;
            }
            if links && let Some((words, url, after)) = worded_link(rest) {
                html.push_str("<a href=\"");
                escape(url, html);
                html.push_str("\">");
                inline(words, marks, false, false, html);
                html.push_str("</a>");
                rest = after;
                continue;
            }
        }
        if own && let Some(after) = rest.strip_prefix("***") {
            // Bold and italic together: `***both***`, `**bold *and italic***`.
            let top = &open[open.len().saturating_sub(2)..];
            if top.len() == 2 && top.contains(&Span::Bold) && top.contains(&Span::Italic) {
                for _ in 0..2 {
                    let span = open[open.len() - 1];
                    toggle(&mut open, span, html);
                }
                rest = after;
                continue;
            }
            if open.is_empty() && has_unescaped(after, "***") {
                toggle(&mut open, Span::Bold, html);
                toggle(&mut open, Span::Italic, html);
                rest = after;
                continue;
            }
        }
        if let Some(after) = rest.strip_prefix("**")
            && (open.contains(&Span::Bold) || contains_mark(after, "**", own))
        {
            toggle(&mut open, Span::Bold, html);
            rest = after;
            continue;
        }
        if let Some(after) = rest.strip_prefix('*')
            && (open.contains(&Span::Italic) || closes_italic(after, own))
        {
            toggle(&mut open, Span::Italic, html);
            rest = after;
            continue;
        }
        if links && let Some(len) = link_len(rest) {
            let (url, after) = rest.split_at(len);
            html.push_str("<a href=\"");
            escape(url, html);
            html.push_str("\">");
            escape(url, html);
            html.push_str("</a>");
            rest = after;
            continue;
        }
        let mut chars = rest.chars();
        if let Some(c) = chars.next() {
            escape_char(c, html);
        }
        rest = chars.as_str();
    }
    while let Some(s) = open.pop() {
        let _ = write!(html, "</{}>", s.tag());
    }
}

fn has_unescaped(text: &str, pat: &str) -> bool {
    contains_mark(text, pat, true)
}

/// Whether `pat` is in `text`, not counting a `\`-escaped character when
/// escapes apply.
fn contains_mark(text: &str, pat: &str, own: bool) -> bool {
    if !own {
        return text.contains(pat);
    }
    let mut chars = text.char_indices();
    while let Some((i, c)) = chars.next() {
        if c == '\\' {
            chars.next();
        } else if text[i..].starts_with(pat) {
            return true;
        }
    }
    false
}

/// Whether a single `*` later on the line can close an italic run opened
/// here (one that is not part of a `**`).
fn closes_italic(after: &str, own: bool) -> bool {
    if after.is_empty() || after.starts_with([' ', '*']) {
        return false;
    }
    let bytes = after.as_bytes();
    let mut i = 1;
    while i < bytes.len() {
        if own && bytes[i] == b'\\' {
            i += 2;
            continue;
        }
        if bytes[i] == b'*'
            && !matches!(bytes[i - 1], b' ' | b'*')
            && !(own && bytes[i - 1] == b'\\')
            && (bytes.get(i + 1) != Some(&b'*') || (own && after[i..].starts_with("***")))
        {
            return true;
        }
        i += 1;
    }
    false
}

/// The length of a bare link at the start of `text`, if one starts there.
fn link_len(text: &str) -> Option<usize> {
    let scheme = if text.starts_with("https://") {
        8
    } else if text.starts_with("http://") {
        7
    } else {
        return None;
    };
    let end = text
        .find(|c: char| c.is_whitespace() || matches!(c, '<' | '>' | '"' | '\''))
        .unwrap_or(text.len());
    // Trailing punctuation belongs to the sentence, not the link.
    let url = text[..end].trim_end_matches(['.', ',', ';', ':', '!', '?', ')', '*']);
    (url.len() > scheme).then_some(url.len())
}

fn escape(text: &str, html: &mut String) {
    for c in text.chars() {
        escape_char(c, html);
    }
}

fn escape_char(c: char, html: &mut String) {
    match c {
        '&' => html.push_str("&amp;"),
        '<' => html.push_str("&lt;"),
        '>' => html.push_str("&gt;"),
        '"' => html.push_str("&quot;"),
        '\'' => html.push_str("&#39;"),
        c => html.push(c),
    }
}

#[cfg(test)]
mod tests {
    use super::{Marks, to_html, to_html_parts, to_plain};

    #[test]
    fn the_assistants_words_carry_no_worded_link() {
        let made = super::literal_links("Pay [here](https://example.net) or \\[keep] C:\\temp");
        assert_eq!(
            made,
            "Pay \\[here\\](https://example.net) or \\[keep\\] C:\\temp"
        );
        let html = to_html(&made, super::Marks::Own);
        assert!(!html.contains(">here</a>"), "{html}");
        assert!(html.contains("[here]"), "{html}");
    }

    fn own(text: &str) -> String {
        to_html(text, Marks::Own)
    }

    #[test]
    fn plain_text_is_escaped_into_paragraphs() {
        assert_eq!(
            own("Hi <b>Ana</b> & co,\nsee you\n\nSam"),
            "<div><p>Hi &lt;b&gt;Ana&lt;/b&gt; &amp; co,<br>see you</p><p>Sam</p></div>"
        );
    }

    #[test]
    fn emphasis_lists_and_quotes() {
        assert_eq!(
            own("**Done**, *mostly*.\n- one\n- two\n1. first\n2. second"),
            "<div><p><strong>Done</strong>, <em>mostly</em>.</p><ul><li>one</li><li>two</li></ul>\
             <ol><li>first</li><li>second</li></ol></div>"
        );
        assert_eq!(
            own("Thanks\n\nOn Tue, Ana wrote:\n> Can we?\n>> Earlier"),
            "<div><p>Thanks</p><p>On Tue, Ana wrote:</p><blockquote><p>Can we?</p>\
             <blockquote><p>Earlier</p></blockquote></blockquote></div>"
        );
    }

    #[test]
    fn unmatched_marks_stay_text() {
        assert_eq!(own("2 * 3 = 6"), "<div><p>2 * 3 = 6</p></div>");
        assert_eq!(own("a ** b"), "<div><p>a ** b</p></div>");
        assert_eq!(own("a ~~ b"), "<div><p>a ~~ b</p></div>");
    }

    #[test]
    fn links_are_http_only_and_escaped() {
        assert_eq!(
            own("See https://example.com/a?b=1&c=2."),
            "<div><p>See <a href=\"https://example.com/a?b=1&amp;c=2\">https://example.com/a?b=1&amp;c=2</a>.</p></div>"
        );
        assert_eq!(
            own("javascript:alert(1) https://x.example\"onmouseover=1"),
            "<div><p>javascript:alert(1) <a href=\"https://x.example\">https://x.example</a>&quot;onmouseover=1</p></div>"
        );
    }

    #[test]
    fn markup_cannot_be_injected() {
        let html = own("<script>alert(1)</script> **done**");
        assert!(html.contains("&lt;script&gt;alert(1)&lt;/script&gt; <strong>done</strong>"));
        assert!(!html.contains("<script"));
    }

    #[test]
    fn a_link_with_words() {
        assert_eq!(
            own("See [the lease](https://example.com/4b)."),
            "<div><p>See <a href=\"https://example.com/4b\">the lease</a>.</p></div>"
        );
        assert_eq!(
            own("Write [me](mailto:ana@example.com)"),
            "<div><p>Write <a href=\"mailto:ana@example.com\">me</a></p></div>"
        );
        assert_eq!(
            own("[**the** lease](https://example.com/4b)"),
            "<div><p><a href=\"https://example.com/4b\"><strong>the</strong> lease</a></p></div>"
        );
    }

    #[test]
    fn a_link_never_nests_in_a_link() {
        assert_eq!(
            own("[a [b](https://x.example) c](https://y.example)"),
            "<div><p><a href=\"https://x.example\">a [b</a> c](<a href=\"https://y.example\">https://y.example</a>)</p></div>"
        );
        assert_eq!(
            own("[see https://x.example now](https://y.example)"),
            "<div><p><a href=\"https://y.example\">see https://x.example now</a></p></div>"
        );
    }

    #[test]
    fn only_web_and_mail_addresses_are_linked() {
        for bad in [
            "[click](javascript:alert(1))",
            "[click](data:text/html,x)",
            "[click](ftp://example.com/a)",
            "[click](https://exa mple.com)",
            "[click](https://)",
            "[](https://example.com)",
        ] {
            let html = own(bad);
            assert!(!html.contains("<a href=\"javascript"), "{bad}: {html}");
            assert!(!html.contains("<a href=\"data"), "{bad}: {html}");
            assert!(!html.contains("<a href=\"ftp"), "{bad}: {html}");
        }
        assert_eq!(
            own("[click](javascript:alert(1))"),
            "<div><p>[click](javascript:alert(1))</p></div>"
        );
        assert_eq!(
            own("[click](https://x.example\"onmouseover=1)"),
            "<div><p>[click](<a href=\"https://x.example\">https://x.example</a>&quot;onmouseover=1)</p></div>"
        );
    }

    #[test]
    fn strikethrough() {
        assert_eq!(
            own("rent on the ~~5th~~ 1st"),
            "<div><p>rent on the <s>5th</s> 1st</p></div>"
        );
        assert_eq!(
            own("**a ~~b** c~~"),
            "<div><p><strong>a <s>b</s></strong><s> c</s></p></div>"
        );
    }

    #[test]
    fn an_escape_makes_a_mark_plain() {
        assert_eq!(own("2 \\* 3"), "<div><p>2 * 3</p></div>");
        assert_eq!(own("\\*not\\* italic"), "<div><p>*not* italic</p></div>");
        assert_eq!(own("\\~~x~~"), "<div><p>~~x~~</p></div>");
        assert_eq!(
            own("\\[a\\](https://x.example)"),
            "<div><p>[a](<a href=\"https://x.example\">https://x.example</a>)</p></div>"
        );
        assert_eq!(own("a \\\\ b"), "<div><p>a \\ b</p></div>");
        assert_eq!(own("\\- not a list"), "<div><p>- not a list</p></div>");
        assert_eq!(own("\\> not a quote"), "<div><p>&gt; not a quote</p></div>");
        assert_eq!(own("1\\. not a list"), "<div><p>1. not a list</p></div>");
        assert_eq!(
            own("> \\- inside a quote"),
            "<div><blockquote><p>- inside a quote</p></blockquote></div>"
        );
    }

    #[test]
    fn a_backslash_before_anything_else_stays() {
        assert_eq!(own("C:\\temp"), "<div><p>C:\\temp</p></div>");
        assert_eq!(own("a \\- b"), "<div><p>a \\- b</p></div>");
    }

    #[test]
    fn the_quoted_history_keeps_fewer_marks() {
        assert_eq!(
            to_html("[your bank](https://example.net)", Marks::Quoted),
            "<div><p>[your bank](<a href=\"https://example.net\">https://example.net</a>)</p></div>"
        );
        assert_eq!(
            to_html("~~x~~ \\* **y**", Marks::Quoted),
            "<div><p>~~x~~ \\* <strong>y</strong></p></div>"
        );
        assert_eq!(
            to_html_parts(&[
                ("See [a](https://x.example)", Marks::Own),
                (
                    "On Tue, Ana wrote:\n> [b](https://y.example)",
                    Marks::Quoted
                ),
            ]),
            "<div><p>See <a href=\"https://x.example\">a</a></p><p>On Tue, Ana wrote:</p>\
             <blockquote><p>[b](<a href=\"https://y.example\">https://y.example</a>)</p></blockquote></div>"
        );
    }

    #[test]
    fn plain_text_writes_links_and_drops_escapes() {
        assert_eq!(
            to_plain("See [the lease](https://example.com/4b)."),
            "See the lease <https://example.com/4b>."
        );
        assert_eq!(to_plain("2 \\* 3"), "2 * 3");
        assert_eq!(
            to_plain("**a** ~~b~~\n- x\n- \\* y"),
            "**a** ~~b~~\n- x\n- * y"
        );
        assert_eq!(to_plain("\\- a\n> \\- b\n1\\. c"), "- a\n> - b\n1. c");
        assert_eq!(to_plain("C:\\temp"), "C:\\temp");
        assert_eq!(
            to_plain("[click](javascript:alert(1))"),
            "[click](javascript:alert(1))"
        );
        assert_eq!(
            to_plain("[**a** \\]](mailto:a@example.com)"),
            "**a** ] <mailto:a@example.com>"
        );
    }

    #[test]
    fn a_message_renders_body_and_quote_apart() {
        let (html, text) = super::message(
            "See [the lease](https://example.com/4b).\n",
            "On Tue, Ana wrote:\n> [your bank](https://example.net)",
        );
        assert_eq!(
            html,
            "<div><p>See <a href=\"https://example.com/4b\">the lease</a>.</p><p>On Tue, Ana wrote:</p>\
             <blockquote><p>[your bank](<a href=\"https://example.net\">https://example.net</a>)</p></blockquote></div>"
        );
        assert_eq!(
            text,
            "See the lease <https://example.com/4b>.\n\nOn Tue, Ana wrote:\n> [your bank](https://example.net)"
        );
        let (html, text) = super::message("2 \\* 3", "  ");
        assert_eq!(
            (html.as_str(), text.as_str()),
            ("<div><p>2 * 3</p></div>", "2 * 3")
        );
    }

    /// The cases the client's tests read too (`marks.json`), so the two
    /// sides cannot drift apart.
    #[test]
    fn the_shared_fixture() {
        let cases: serde_json::Value =
            serde_json::from_str(include_str!("../../tests/fixtures/marks.json")).unwrap();
        for case in cases.as_array().unwrap() {
            let marks = case["marks"].as_str().unwrap();
            assert_eq!(
                to_html(marks, Marks::Own),
                case["html"].as_str().unwrap(),
                "html of {}",
                case["name"]
            );
            assert_eq!(
                to_plain(marks),
                case["plain"].as_str().unwrap(),
                "plain of {}",
                case["name"]
            );
        }
    }
}
