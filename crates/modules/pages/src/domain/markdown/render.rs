//! A page's body as Markdown, for Scout to read.

use yrs::{
    Any, Doc, Out, ReadTxn, Text, Transact, XmlElementRef, XmlFragment, XmlOut, XmlTextRef,
    types::text::YChange,
};

use crate::domain::{
    document::ROOT,
    notion::tree::{attributes, text_attribute},
};

/// A page read as text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rendered {
    /// The Markdown: the title as a heading, then the blocks in order.
    pub markdown: String,
    /// The text was cut at a block boundary because it reached the cap.
    pub truncated: bool,
}

/// `doc` as Markdown under `title`, of at most `cap` bytes, cut after the
/// last block that fits. Mentions and embeds are written as `[type:id]` and
/// never resolved.
pub fn render(doc: &Doc, title: &str, cap: usize) -> Rendered {
    let root = doc.get_or_insert_xml_fragment(ROOT);
    let txn = doc.transact();
    let mut markdown = if title.is_empty() {
        String::new()
    } else {
        format!("# {title}")
    };
    cut(&mut markdown, cap);
    let mut truncated = false;
    for child in root.children(&txn) {
        let XmlOut::Element(element) = child else {
            continue;
        };
        let block = block(&txn, &element);
        if block.is_empty() {
            continue;
        }
        let separator = if markdown.is_empty() { "" } else { "\n\n" };
        if markdown.len() + separator.len() + block.len() > cap {
            // A block that alone is over the cap still gives its start when
            // nothing else was read.
            if markdown.is_empty() {
                markdown = block;
                cut(&mut markdown, cap);
            }
            truncated = true;
            break;
        }
        markdown.push_str(separator);
        markdown.push_str(&block);
    }
    Rendered {
        markdown,
        truncated,
    }
}

/// `text` cut to at most `cap` bytes, at a character boundary.
fn cut(text: &mut String, cap: usize) {
    if text.len() > cap {
        let mut end = cap;
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        text.truncate(end);
    }
}

fn whole(attributes: &std::collections::BTreeMap<String, Any>, key: &str) -> Option<i64> {
    match attributes.get(key) {
        #[allow(
            clippy::cast_possible_truncation,
            reason = "a heading level or a list start, checked small by the schema"
        )]
        Some(Any::Number(number)) if number.is_finite() => Some(*number as i64),
        Some(Any::BigInt(number)) => Some(*number),
        _ => None,
    }
}

/// One block as Markdown (no trailing blank line).
fn block<T: ReadTxn>(txn: &T, element: &XmlElementRef) -> String {
    let attributes = attributes(txn, element);
    let tag = element.tag().to_string();
    match tag.as_str() {
        "paragraph" => inline(txn, element),
        "heading" => {
            let level = whole(&attributes, "level").unwrap_or(1).clamp(1, 3);
            let hashes = "#".repeat(usize::try_from(level).unwrap_or(1));
            format!("{hashes} {}", inline(txn, element))
        }
        "codeBlock" => {
            let language = text_attribute(&attributes, "language").unwrap_or_default();
            let language = if language == "plaintext" {
                ""
            } else {
                language.as_str()
            };
            format!("```{language}\n{}\n```", inline(txn, element))
        }
        "bulletList" | "orderedList" | "taskList" => list(txn, element, &tag, &attributes),
        "blockquote" | "callout" | "toggle" => {
            let inner = blocks(txn, element);
            inner
                .lines()
                .map(|line| {
                    if line.is_empty() {
                        ">".to_owned()
                    } else {
                        format!("> {line}")
                    }
                })
                .collect::<Vec<_>>()
                .join("\n")
        }
        "horizontalRule" => "---".to_owned(),
        "image" | "file" => {
            let name = text_attribute(&attributes, "name").unwrap_or_default();
            format!("[{tag}: {name}]")
        }
        "pageLink" => format!(
            "[pages.page:{}]",
            text_attribute(&attributes, "page").unwrap_or_default()
        ),
        "tableView" => format!(
            "[pages.page:{}]",
            text_attribute(&attributes, "table").unwrap_or_default()
        ),
        "embed" => reference(&attributes),
        "external" => format!(
            "[external: {}]",
            text_attribute(&attributes, "kind").unwrap_or_default()
        ),
        // Notion's version of a block we disagree about: not the page's own
        // words.
        _ => String::new(),
    }
}

/// The blocks inside `element`, one blank line apart.
fn blocks<T: ReadTxn>(txn: &T, element: &XmlElementRef) -> String {
    element
        .children(txn)
        .filter_map(|child| match child {
            XmlOut::Element(inner) => Some(block(txn, &inner)),
            _ => None,
        })
        .filter(|rendered| !rendered.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// A list: each item under its marker, what follows indented under it.
fn list<T: ReadTxn>(
    txn: &T,
    element: &XmlElementRef,
    tag: &str,
    attributes: &std::collections::BTreeMap<String, Any>,
) -> String {
    let start = whole(attributes, "start").unwrap_or(1);
    let mut lines: Vec<String> = Vec::new();
    let mut number = start;
    for child in element.children(txn) {
        let XmlOut::Element(item) = child else {
            continue;
        };
        let held = self::attributes(txn, &item);
        let marker = match tag {
            "orderedList" => {
                let marker = format!("{number}. ");
                number += 1;
                marker
            }
            "taskList" => {
                if matches!(held.get("checked"), Some(Any::Bool(true))) {
                    "- [x] ".to_owned()
                } else {
                    "- [ ] ".to_owned()
                }
            }
            _ => "- ".to_owned(),
        };
        let inner = blocks(txn, &item);
        let indent = " ".repeat(marker.len());
        for (at, line) in inner.lines().enumerate() {
            if at == 0 {
                lines.push(format!("{marker}{line}"));
            } else if line.is_empty() {
                lines.push(String::new());
            } else {
                lines.push(format!("{indent}{line}"));
            }
        }
        if inner.is_empty() {
            lines.push(marker.trim_end().to_owned());
        }
    }
    lines.join("\n")
}

fn reference(attributes: &std::collections::BTreeMap<String, Any>) -> String {
    format!(
        "[{}:{}]",
        text_attribute(attributes, "ref_type").unwrap_or_default(),
        text_attribute(attributes, "ref_id").unwrap_or_default()
    )
}

/// The text inside `element`: runs with their marks, mentions as references.
fn inline<T: ReadTxn>(txn: &T, element: &XmlElementRef) -> String {
    let mut out = String::new();
    for child in element.children(txn) {
        match child {
            XmlOut::Text(run) => out.push_str(&runs(txn, &run)),
            XmlOut::Element(inner) => match &**inner.tag() {
                "mention" => out.push_str(&reference(&attributes(txn, &inner))),
                "hardBreak" => out.push('\n'),
                _ => {}
            },
            XmlOut::Fragment(_) => {}
        }
    }
    out
}

fn runs<T: ReadTxn>(txn: &T, run: &XmlTextRef) -> String {
    let mut out = String::new();
    for chunk in run.diff(txn, YChange::identity) {
        let Out::Any(Any::String(text)) = &chunk.insert else {
            continue;
        };
        let mut shown = text.to_string();
        if let Some(marks) = &chunk.attributes {
            let on = |name: &str| {
                marks.iter().any(|(key, value)| {
                    key.split("--").next() == Some(name)
                        && !matches!(value, Any::Null | Any::Undefined | Any::Bool(false))
                })
            };
            if on("code") {
                shown = format!("`{shown}`");
            }
            if on("strike") {
                shown = format!("~~{shown}~~");
            }
            if on("italic") {
                shown = format!("*{shown}*");
            }
            if on("bold") {
                shown = format!("**{shown}**");
            }
            let href = marks.iter().find_map(|(key, value)| match value {
                Any::Map(map) if key.split("--").next() == Some("link") => match map.get("href") {
                    Some(Any::String(href)) => Some(href.to_string()),
                    _ => None,
                },
                _ => None,
            });
            if let Some(href) = href {
                shown = format!("[{shown}]({href})");
            }
        }
        out.push_str(&shown);
    }
    out
}
