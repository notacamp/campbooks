//! Markdown to blocks: a fixed subset, read line by line.

use campbooks_kernel::text;
use uuid::Uuid;

use crate::domain::{
    document::LANGUAGES,
    notion::model::{Block, Kind, Run, normalize},
};

/// How deep emphasis and links may nest.
const MAX_DEPTH: usize = 6;

/// `markdown` as blocks, each with a fresh stable id. Never fails: what is
/// not understood is text.
pub fn parse(markdown: &str) -> Vec<Block> {
    let lines: Vec<String> = markdown
        .lines()
        .map(|line| text::displayable(&line.replace('\t', "    ")))
        .collect();
    let mut reader = Reader::default();
    let mut at = 0;
    while at < lines.len() {
        let line = lines[at].as_str();
        at += 1;
        let trimmed = line.trim();
        if let Some(info) = fence_info(line) {
            reader.flush();
            let mut code: Vec<&str> = Vec::new();
            while at < lines.len() {
                let next = lines[at].as_str();
                at += 1;
                if next.trim_end() == "```" || next.trim() == "```" {
                    break;
                }
                code.push(next);
            }
            reader.out.push(code_block(info, &code));
        } else if trimmed.is_empty() {
            reader.flush();
        } else if is_rule(trimmed) {
            reader.flush();
            reader.out.push(Block::new(Uuid::now_v7(), Kind::Divider));
        } else if let Some((level, rest)) = heading(trimmed) {
            reader.flush();
            let mut block = Block::new(Uuid::now_v7(), Kind::Heading);
            block.level = Some(level);
            block.runs = inline(rest);
            reader.out.push(block);
        } else if let Some(rest) = quote_line(line) {
            reader.quote(rest);
        } else if let Some((kind, checked, rest)) = list_marker(line) {
            reader.item(kind, checked, rest);
        } else {
            reader.text(line, trimmed);
        }
    }
    reader.flush();
    reader.out
}

/// What is being read and not yet a block.
#[derive(Debug, Default)]
enum Open {
    #[default]
    Nothing,
    Paragraph(Vec<String>),
    /// Paragraphs of a quote: the one being read last.
    Quote(Vec<Vec<String>>),
    Item {
        kind: Kind,
        checked: bool,
        parts: Vec<String>,
    },
}

#[derive(Debug, Default)]
struct Reader {
    out: Vec<Block>,
    open: Open,
}

impl Reader {
    /// Close what is open as a block.
    fn flush(&mut self) {
        match std::mem::take(&mut self.open) {
            Open::Nothing => {}
            Open::Paragraph(lines) => {
                let mut block = Block::new(Uuid::now_v7(), Kind::Paragraph);
                block.runs = inline(&lines.join(" "));
                self.out.push(block);
            }
            Open::Quote(paragraphs) => {
                let mut paragraphs = paragraphs.into_iter().filter(|p| !p.is_empty());
                let mut block = Block::new(Uuid::now_v7(), Kind::Quote);
                if let Some(lead) = paragraphs.next() {
                    block.runs = inline(&lead.join(" "));
                }
                for rest in paragraphs {
                    let mut child = Block::new(Uuid::now_v7(), Kind::Paragraph);
                    child.runs = inline(&rest.join(" "));
                    block.children.push(child);
                }
                self.out.push(block);
            }
            Open::Item {
                kind,
                checked,
                parts,
            } => {
                let mut block = Block::new(Uuid::now_v7(), kind);
                if kind == Kind::Todo {
                    block.checked = Some(checked);
                }
                block.runs = inline(&parts.join(" "));
                self.out.push(block);
            }
        }
    }

    /// A line of a quote, without its `>`.
    fn quote(&mut self, rest: &str) {
        let content = rest.trim();
        if let Open::Quote(paragraphs) = &mut self.open {
            if content.is_empty() {
                paragraphs.push(Vec::new());
            } else if let Some(last) = paragraphs.last_mut() {
                last.push(content.to_owned());
            }
            return;
        }
        self.flush();
        self.open = Open::Quote(vec![if content.is_empty() {
            Vec::new()
        } else {
            vec![content.to_owned()]
        }]);
    }

    fn item(&mut self, kind: Kind, checked: bool, rest: &str) {
        self.flush();
        self.open = Open::Item {
            kind,
            checked,
            parts: vec![rest.trim().to_owned()],
        };
    }

    /// A line that is none of the others: more of the open paragraph, a
    /// continuation of the open item, or a new paragraph.
    fn text(&mut self, line: &str, trimmed: &str) {
        match &mut self.open {
            Open::Paragraph(lines) => lines.push(trimmed.to_owned()),
            Open::Item { parts, .. } if line.starts_with(' ') => parts.push(trimmed.to_owned()),
            _ => {
                self.flush();
                self.open = Open::Paragraph(vec![trimmed.to_owned()]);
            }
        }
    }
}

/// The info string of a code fence opening `line` (up to three spaces of
/// indent, then three backticks).
fn fence_info(line: &str) -> Option<&str> {
    let indent = line.len() - line.trim_start_matches(' ').len();
    let rest = line[indent..].strip_prefix("```")?;
    (indent <= 3 && !rest.contains('`')).then(|| rest.trim())
}

/// A code block of `info`'s language holding `lines`.
fn code_block(info: &str, lines: &[&str]) -> Block {
    let name = info
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let language = match name.as_str() {
        "js" | "jsx" | "mjs" => "javascript",
        "ts" | "tsx" => "typescript",
        "sh" | "shell" | "zsh" => "bash",
        "py" => "python",
        "yml" => "yaml",
        "c++" => "cpp",
        "c#" => "csharp",
        "md" => "markdown",
        "rs" => "rust",
        "rb" => "ruby",
        other => other,
    };
    let mut block = Block::new(Uuid::now_v7(), Kind::Code);
    block.language = Some(
        if LANGUAGES.contains(&language) {
            language
        } else {
            "plaintext"
        }
        .to_owned(),
    );
    let code = lines.join("\n");
    if !code.is_empty() {
        block.runs = vec![Run::plain(&code)];
    }
    block
}

/// Three or more of one of `-`, `*`, `_`, with spaces allowed between.
fn is_rule(trimmed: &str) -> bool {
    let mut marks = trimmed.chars().filter(|c| *c != ' ');
    let Some(first) = marks.next().filter(|c| matches!(c, '-' | '*' | '_')) else {
        return false;
    };
    let mut count = 1;
    for c in marks {
        if c != first {
            return false;
        }
        count += 1;
    }
    count >= 3
}

/// A heading: one to six `#`, a space, the words. Deeper than three is
/// three.
fn heading(trimmed: &str) -> Option<(u8, &str)> {
    let hashes = trimmed.chars().take_while(|c| *c == '#').count();
    if !(1..=6).contains(&hashes) {
        return None;
    }
    let rest = &trimmed[hashes..];
    if !rest.is_empty() && !rest.starts_with(' ') {
        return None;
    }
    let words = rest.trim().trim_end_matches('#').trim_end();
    let level = u8::try_from(hashes.min(3)).unwrap_or(3);
    Some((level, words))
}

/// A quote line: up to three spaces, `>`, then the rest.
fn quote_line(line: &str) -> Option<&str> {
    let indent = line.len() - line.trim_start_matches(' ').len();
    let rest = line[indent..].strip_prefix('>')?;
    (indent <= 3).then(|| rest.strip_prefix(' ').unwrap_or(rest))
}

/// A list item line: its kind, whether a to-do is done, and its words.
/// Indentation is read but not kept: a list is one level.
fn list_marker(line: &str) -> Option<(Kind, bool, &str)> {
    let body = line.trim_start_matches(' ');
    let mut chars = body.chars();
    let first = chars.next()?;
    if matches!(first, '-' | '*' | '+') {
        let rest = body[1..].strip_prefix(' ')?;
        let rest = rest.trim_start();
        for (mark, done) in [("[ ]", false), ("[x]", true), ("[X]", true)] {
            if let Some(after) = rest.strip_prefix(mark)
                && (after.is_empty() || after.starts_with(' '))
            {
                return Some((Kind::Todo, done, after.trim_start()));
            }
        }
        return Some((Kind::BulletedItem, false, rest));
    }
    let digits = body.chars().take_while(char::is_ascii_digit).count();
    if (1..=9).contains(&digits) {
        let after = &body[digits..];
        let rest = after
            .strip_prefix('.')
            .or_else(|| after.strip_prefix(')'))?
            .strip_prefix(' ')?;
        return Some((Kind::NumberedItem, false, rest.trim_start()));
    }
    None
}

// ---- inline --------------------------------------------------------------

/// What marks the text being read carries.
#[derive(Debug, Clone, Default)]
struct Marks {
    bold: bool,
    italic: bool,
    strike: bool,
    link: Option<String>,
}

/// `text` as runs.
fn inline(text: &str) -> Vec<Run> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    read(&chars, &Marks::default(), &mut out, 0);
    normalize(out)
}

/// Delimiters, for remembering where a search for a closer already failed.
#[derive(Debug, Clone, Copy)]
enum Delimiter {
    Stars2,
    Unders2,
    Tildes,
    Star,
    Under,
    Code,
    Bracket,
}

#[derive(Debug, Default)]
struct Failed([Option<usize>; 7]);

impl Failed {
    /// Whether a search for `which` from `from` already found nothing: a
    /// closer does not depend on its opener, so later searches fail too.
    fn already(&self, which: Delimiter, from: usize) -> bool {
        self.0[which as usize].is_some_and(|failed| from >= failed)
    }

    fn note(&mut self, which: Delimiter, from: usize) {
        let slot = &mut self.0[which as usize];
        *slot = Some(slot.map_or(from, |failed| failed.min(from)));
    }
}

fn run(text: &str, marks: &Marks, code: bool) -> Run {
    Run {
        text: text.to_owned(),
        bold: marks.bold,
        italic: marks.italic,
        strike: marks.strike,
        code,
        link: marks.link.clone(),
        page: None,
    }
}

/// Whether `chars[at..]` starts with `pattern`.
fn starts(chars: &[char], at: usize, pattern: &[char]) -> bool {
    chars.get(at..at + pattern.len()) == Some(pattern)
}

/// The index of the first valid closer `pattern` at or after `from`: one
/// not escaped and not right after a space. The words between must not be
/// empty.
fn closer(chars: &[char], from: usize, pattern: &[char]) -> Option<usize> {
    let mut at = from;
    while at + pattern.len() <= chars.len() {
        if chars[at] == '\\' {
            at += 2;
            continue;
        }
        if starts(chars, at, pattern) && at > from && chars[at - 1] != ' ' {
            // A single `*` or `_` is not the first half of a double one.
            let single = pattern.len() == 1;
            if !(single && chars.get(at + 1) == Some(&pattern[0])) {
                return Some(at);
            }
            at += 2;
            continue;
        }
        at += 1;
    }
    None
}

/// Whether a `_` at `at` may open or close emphasis: not inside a word.
fn word_edge(chars: &[char], at: usize, length: usize) -> bool {
    let before = at.checked_sub(1).and_then(|i| chars.get(i));
    let after = chars.get(at + length);
    before.is_none_or(|c| !c.is_alphanumeric()) || after.is_none_or(|c| !c.is_alphanumeric())
}

#[allow(
    clippy::too_many_lines,
    reason = "one arm per piece of syntax, read top to bottom"
)]
fn read(chars: &[char], marks: &Marks, out: &mut Vec<Run>, depth: usize) {
    let mut buffer = String::new();
    let mut failed = Failed::default();
    let mut at = 0;
    macro_rules! flush {
        () => {
            if !buffer.is_empty() {
                out.push(run(&buffer, marks, false));
                buffer.clear();
            }
        };
    }
    'scan: while at < chars.len() {
        let c = chars[at];
        // An escaped punctuation mark is itself.
        if c == '\\'
            && let Some(next) = chars.get(at + 1)
            && next.is_ascii_punctuation()
        {
            buffer.push(*next);
            at += 2;
            continue;
        }
        if depth < MAX_DEPTH {
            // `code`: no other syntax inside.
            if c == '`' && !failed.already(Delimiter::Code, at + 1) {
                if let Some(end) = chars[at + 1..].iter().position(|x| *x == '`') {
                    let end = at + 1 + end;
                    if end > at + 1 {
                        flush!();
                        let code: String = chars[at + 1..end].iter().collect();
                        out.push(run(&code, marks, true));
                        at = end + 1;
                        continue;
                    }
                } else {
                    failed.note(Delimiter::Code, at + 1);
                }
            }
            // **bold** and __bold__, ~~strike~~.
            for (pattern, which) in [
                (['*', '*'], Delimiter::Stars2),
                (['_', '_'], Delimiter::Unders2),
                (['~', '~'], Delimiter::Tildes),
            ] {
                if starts(chars, at, &pattern)
                    && chars.get(at + 2).is_some_and(|x| *x != ' ')
                    && (pattern[0] != '_' || word_edge(chars, at, 2))
                    && !failed.already(which, at + 2)
                {
                    match closer(chars, at + 2, &pattern) {
                        Some(end) => {
                            flush!();
                            let mut inner = marks.clone();
                            if pattern[0] == '~' {
                                inner.strike = true;
                            } else {
                                inner.bold = true;
                            }
                            read(&chars[at + 2..end], &inner, out, depth + 1);
                            at = end + 2;
                            continue 'scan;
                        }
                        None => failed.note(which, at + 2),
                    }
                }
            }
            // *italic* and _italic_.
            if (c == '*' || c == '_')
                && chars.get(at + 1).is_some_and(|x| *x != ' ' && *x != c)
                && (c != '_' || word_edge(chars, at, 1))
                && !failed.already(which_single(c), at + 1)
            {
                match closer(chars, at + 1, &[c]) {
                    Some(end) if c != '_' || word_edge(chars, end, 1) => {
                        flush!();
                        let mut inner = marks.clone();
                        inner.italic = true;
                        read(&chars[at + 1..end], &inner, out, depth + 1);
                        at = end + 1;
                        continue;
                    }
                    Some(_) => {}
                    None => failed.note(which_single(c), at + 1),
                }
            }
            // [words](address)
            if c == '[' && marks.link.is_none() && !failed.already(Delimiter::Bracket, at + 1) {
                match link(chars, at) {
                    Linked::Link(end, after, address) => {
                        flush!();
                        let mut inner = marks.clone();
                        inner.link = safe_address(&address);
                        read(&chars[at + 1..end], &inner, out, depth + 1);
                        at = after;
                        continue;
                    }
                    Linked::NoClose => failed.note(Delimiter::Bracket, at + 1),
                    Linked::NotALink => {}
                }
            }
        }
        buffer.push(c);
        at += 1;
    }
    flush!();
}

/// The delimiter a single `*` or `_` is.
const fn which_single(c: char) -> Delimiter {
    if c == '*' {
        Delimiter::Star
    } else {
        Delimiter::Under
    }
}

/// What the `[` at some index starts.
enum Linked {
    /// A link: where its words end (the `]`), the index after its `)`, and
    /// its address.
    Link(usize, usize, String),
    /// Brackets, but no link.
    NotALink,
    /// No `]` after it at all, so none after a later `[` either.
    NoClose,
}

/// A link starting at the `[` at `at`.
fn link(chars: &[char], at: usize) -> Linked {
    let mut end = at + 1;
    let mut depth = 0usize;
    while end < chars.len() {
        match chars[end] {
            '\\' => end += 1,
            '[' => depth += 1,
            ']' if depth == 0 => break,
            ']' => depth -= 1,
            _ => {}
        }
        end += 1;
    }
    if end >= chars.len() {
        return Linked::NoClose;
    }
    if end == at + 1 || chars.get(end + 1) != Some(&'(') {
        return Linked::NotALink;
    }
    // The address may hold balanced parentheses: `(javascript:alert(1))`.
    let mut open = 0usize;
    let mut close = None;
    for (at, c) in chars.iter().enumerate().skip(end + 2) {
        match c {
            '(' => open += 1,
            ')' if open == 0 => {
                close = Some(at);
                break;
            }
            ')' => open -= 1,
            _ => {}
        }
    }
    let Some(close) = close else {
        return Linked::NotALink;
    };
    let inside: String = chars[end + 2..close].iter().collect();
    // `(address "title")`: the address is up to the first space.
    let address = inside.split_whitespace().next().unwrap_or_default();
    Linked::Link(end, close + 1, address.to_owned())
}

/// The address as a link keeps it: `https`, `http` or `mailto` only.
fn safe_address(address: &str) -> Option<String> {
    let lower = address.to_ascii_lowercase();
    let allowed = ["https://", "http://", "mailto:"]
        .iter()
        .any(|scheme| lower.starts_with(scheme));
    (allowed && address.len() <= 2048 && !address.chars().any(char::is_control))
        .then(|| address.to_owned())
}
