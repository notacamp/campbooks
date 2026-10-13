//! The basic formatting a member may use in a message, rendered to the HTML
//! alternative of what they send.
//!
//! Safe by construction: every character is escaped first, and only these
//! marks become markup: `**bold**`, `*italic*`, lines starting `- ` or `* `
//! (a bulleted list), `1. ` (a numbered list), `> ` (a quote, nested by
//! repeating it), and bare `http://` / `https://` links. Blank lines
//! separate paragraphs; single line breaks are kept. There is no way to
//! write raw HTML.

use std::fmt::Write as _;

/// `text` as HTML.
pub fn to_html(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let mut html = String::from("<div>");
    blocks(&lines, &mut html);
    html.push_str("</div>");
    html
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

fn blocks(lines: &[&str], html: &mut String) {
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        if line.trim().is_empty() {
            i += 1;
        } else if is_quote(line) {
            let end = run(lines, i, |l| is_quote(l));
            html.push_str("<blockquote>");
            blocks(&unquote(&lines[i..end]), html);
            html.push_str("</blockquote>");
            i = end;
        } else if bullet(line).is_some() {
            let end = run(lines, i, |l| bullet(l).is_some());
            list("ul", lines[i..end].iter().filter_map(|l| bullet(l)), html);
            i = end;
        } else if numbered(line).is_some() {
            let end = run(lines, i, |l| numbered(l).is_some());
            list("ol", lines[i..end].iter().filter_map(|l| numbered(l)), html);
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
                inline(line, html);
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

fn list<'a>(tag: &str, items: impl Iterator<Item = &'a str>, html: &mut String) {
    let _ = write!(html, "<{tag}>");
    for item in items {
        html.push_str("<li>");
        inline(item, html);
        html.push_str("</li>");
    }
    let _ = write!(html, "</{tag}>");
}

/// One line: emphasis and links, everything else escaped.
fn inline(line: &str, html: &mut String) {
    let mut rest = line;
    let mut bold = false;
    let mut italic = false;
    while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix("**") {
            if bold || after.contains("**") {
                html.push_str(if bold { "</strong>" } else { "<strong>" });
                bold = !bold;
                rest = after;
                continue;
            }
        }
        if let Some(after) = rest.strip_prefix('*') {
            if italic || closes_italic(after) {
                html.push_str(if italic { "</em>" } else { "<em>" });
                italic = !italic;
                rest = after;
                continue;
            }
        }
        if let Some(len) = link_len(rest) {
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
    if italic {
        html.push_str("</em>");
    }
    if bold {
        html.push_str("</strong>");
    }
}

/// Whether a single `*` later on the line can close an italic run opened
/// here (one that is not part of a `**`).
fn closes_italic(after: &str) -> bool {
    if after.is_empty() || after.starts_with([' ', '*']) {
        return false;
    }
    let bytes = after.as_bytes();
    (1..bytes.len()).any(|i| {
        bytes[i] == b'*' && !matches!(bytes[i - 1], b' ' | b'*') && bytes.get(i + 1) != Some(&b'*')
    })
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
    use super::to_html;

    #[test]
    fn plain_text_is_escaped_into_paragraphs() {
        assert_eq!(
            to_html("Hi <b>Ana</b> & co,\nsee you\n\nSam"),
            "<div><p>Hi &lt;b&gt;Ana&lt;/b&gt; &amp; co,<br>see you</p><p>Sam</p></div>"
        );
    }

    #[test]
    fn emphasis_lists_and_quotes() {
        assert_eq!(
            to_html("**Done**, *mostly*.\n- one\n- two\n1. first\n2. second"),
            "<div><p><strong>Done</strong>, <em>mostly</em>.</p><ul><li>one</li><li>two</li></ul>\
             <ol><li>first</li><li>second</li></ol></div>"
        );
        assert_eq!(
            to_html("Thanks\n\nOn Tue, Ana wrote:\n> Can we?\n>> Earlier"),
            "<div><p>Thanks</p><p>On Tue, Ana wrote:</p><blockquote><p>Can we?</p>\
             <blockquote><p>Earlier</p></blockquote></blockquote></div>"
        );
    }

    #[test]
    fn unmatched_marks_stay_text() {
        assert_eq!(to_html("2 * 3 = 6"), "<div><p>2 * 3 = 6</p></div>");
        assert_eq!(to_html("a ** b"), "<div><p>a ** b</p></div>");
    }

    #[test]
    fn links_are_http_only_and_escaped() {
        assert_eq!(
            to_html("See https://example.com/a?b=1&c=2."),
            "<div><p>See <a href=\"https://example.com/a?b=1&amp;c=2\">https://example.com/a?b=1&amp;c=2</a>.</p></div>"
        );
        assert_eq!(
            to_html("javascript:alert(1) https://x.example\"onmouseover=1"),
            "<div><p>javascript:alert(1) <a href=\"https://x.example\">https://x.example</a>&quot;onmouseover=1</p></div>"
        );
    }

    #[test]
    fn markup_cannot_be_injected() {
        let html = to_html("<script>alert(1)</script> **done**");
        assert!(html.contains("&lt;script&gt;alert(1)&lt;/script&gt; <strong>done</strong>"));
        assert!(!html.contains("<script"));
    }
}
