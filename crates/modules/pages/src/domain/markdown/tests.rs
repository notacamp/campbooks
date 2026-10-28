//! Markdown in and out: a fixed subset in, text for Scout out, and nothing
//! that passes through is anything the block schema refuses.

use std::time::{Duration, Instant};

use campbooks_kernel::RecordRef;
use yrs::{Any, Doc, Xml, XmlElementPrelim, XmlFragment, XmlTextPrelim};

use super::{MAX_DEPTH, MAX_TEXT_BYTES, parse, parse_counted, render};
use crate::{
    domain::{
        append, document,
        notion::model::{Block, Kind},
    },
    testkit::docs::{edit, s},
};

fn kinds(blocks: &[Block]) -> Vec<Kind> {
    blocks.iter().map(|block| block.kind).collect()
}

fn text(block: &Block) -> String {
    block.text()
}

/// `markdown` appended to an empty page, checked against the schema.
fn page_of(markdown: &str) -> Doc {
    let doc = Doc::new();
    let added = append::append(&doc, &parse(markdown), &[]).unwrap();
    document::apply(&doc, &added.update).unwrap();
    document::inspect(&doc).unwrap();
    doc
}

#[test]
fn the_blocks_of_the_subset() {
    let blocks = parse(
        "# Kickoff notes\n\nWe agreed on the scope.\nIt is small.\n\n## Next\n\n\
         - call Acme\n- send the agenda\n\n1. draft\n2. review\n\n\
         - [ ] book the room\n- [x] invite Ana\n\n> Quote line\n> continues\n\n---\n\n\
         ```rust\nfn main() {}\n```\n",
    );
    assert_eq!(
        kinds(&blocks),
        [
            Kind::Heading,
            Kind::Paragraph,
            Kind::Heading,
            Kind::BulletedItem,
            Kind::BulletedItem,
            Kind::NumberedItem,
            Kind::NumberedItem,
            Kind::Todo,
            Kind::Todo,
            Kind::Quote,
            Kind::Divider,
            Kind::Code,
        ]
    );
    assert_eq!(blocks[0].level, Some(1));
    assert_eq!(blocks[2].level, Some(2));
    // Lines of one paragraph are one paragraph.
    assert_eq!(text(&blocks[1]), "We agreed on the scope. It is small.");
    assert_eq!(blocks[7].checked, Some(false));
    assert_eq!(blocks[8].checked, Some(true));
    assert_eq!(text(&blocks[9]), "Quote line continues");
    assert_eq!(blocks[11].language.as_deref(), Some("rust"));
    assert_eq!(text(&blocks[11]), "fn main() {}");
}

#[test]
fn deeper_headings_are_the_third_level_and_unknown_languages_are_plain() {
    let blocks = parse("###### Deep\n\n```brainfuck\n+++\n```");
    assert_eq!(blocks[0].level, Some(3));
    assert_eq!(blocks[1].language.as_deref(), Some("plaintext"));
    // An unclosed fence reads to the end.
    let open = parse("```js\nlet a = 1;\n\nstill code");
    assert_eq!(open.len(), 1);
    assert_eq!(open[0].language.as_deref(), Some("javascript"));
    assert_eq!(text(&open[0]), "let a = 1;\n\nstill code");
}

#[test]
fn a_quote_with_a_blank_line_has_blocks_under_its_first_paragraph() {
    let blocks = parse("> one\n>\n> two");
    assert_eq!(kinds(&blocks), [Kind::Quote]);
    assert_eq!(text(&blocks[0]), "one");
    assert_eq!(blocks[0].children.len(), 1);
    assert_eq!(text(&blocks[0].children[0]), "two");
}

#[test]
fn marks_and_links() {
    let blocks =
        parse("A **bold** and *soft* and ~~gone~~ and `code` and [Acme](https://acme.example/a).");
    let runs = &blocks[0].runs;
    let find = |words: &str| runs.iter().find(|run| run.text == words).unwrap().clone();
    assert!(find("bold").bold);
    assert!(find("soft").italic);
    assert!(find("gone").strike);
    assert!(find("code").code);
    assert_eq!(find("Acme").link.as_deref(), Some("https://acme.example/a"));
    // Snake case is not emphasis.
    assert_eq!(
        text(&parse("a_b_c and 2 * 3 * 4")[0]),
        "a_b_c and 2 * 3 * 4"
    );
    // Escapes are themselves.
    assert_eq!(text(&parse(r"\*not bold\* \[x\]")[0]), "*not bold* [x]");
}

#[test]
fn only_https_http_and_mailto_keep_their_address() {
    for good in [
        "https://example.com/a",
        "HTTP://example.com",
        "mailto:ana@example.com",
    ] {
        let blocks = parse(&format!("[words]({good})"));
        assert_eq!(blocks[0].runs[0].link.as_deref(), Some(good), "{good}");
    }
    for bad in [
        "javascript:alert(1)",
        "JaVaScRiPt:alert(1)",
        "data:text/html;base64,AAAA",
        "vbscript:x",
        "file:///etc/passwd",
        "//evil.example/x",
        "/relative",
        "tel:+351000",
    ] {
        let blocks = parse(&format!("see [the link]({bad}) now"));
        // The words stay; the address does not.
        assert_eq!(text(&blocks[0]), "see the link now", "{bad}");
        assert!(blocks[0].runs.iter().all(|run| run.link.is_none()), "{bad}");
    }
}

#[test]
fn raw_html_is_text() {
    let blocks = parse(
        "<script>alert(1)</script> <b>x</b> <img src=x onerror=alert(1)>\n\n<div>\nblock\n</div>",
    );
    assert_eq!(kinds(&blocks), [Kind::Paragraph, Kind::Paragraph]);
    assert_eq!(
        text(&blocks[0]),
        "<script>alert(1)</script> <b>x</b> <img src=x onerror=alert(1)>"
    );
    assert_eq!(text(&blocks[1]), "<div> block </div>");
    // And it passes the schema as the text it is.
    let doc = page_of("<script>alert(1)</script>");
    assert!(document::inspect(&doc).unwrap().text.contains("<script>"));
}

#[test]
fn unsupported_syntax_is_plain_text() {
    let blocks =
        parse("| a | b |\n|---|---|\n| 1 | 2 |\n\n![alt](https://x.example/i.png)\n\n[^1]: note");
    assert!(blocks.iter().all(|block| block.kind == Kind::Paragraph));
    assert!(text(&blocks[0]).contains("| a | b |"));
}

#[test]
fn hidden_and_control_characters_are_dropped() {
    let blocks = parse("IBAN\u{202e}fdp\u{200b}\u{0007}\tx");
    assert_eq!(text(&blocks[0]), "IBANfdp    x");
}

#[test]
fn nothing_hostile_takes_long_or_breaks_the_schema() {
    let started = Instant::now();
    for hostile in [
        "*".repeat(16 * 1024),
        "[".repeat(16 * 1024),
        "[a](".repeat(3000),
        "`".repeat(16 * 1024),
        "**a ".repeat(4000),
        "_".repeat(16 * 1024),
        "- ".repeat(8000),
        "> ".repeat(8000),
        "#".repeat(16 * 1024),
        "```\n".repeat(4000),
        "[**[**[**[**[**[**[**[**".repeat(600),
    ] {
        let blocks = parse(&hostile);
        let doc = Doc::new();
        let added = append::append(&doc, &blocks, &[]).unwrap();
        document::apply(&doc, &added.update).unwrap();
        document::inspect(&doc).unwrap();
    }
    assert!(
        started.elapsed() < Duration::from_secs(20),
        "{:?}",
        started.elapsed()
    );
}

#[test]
fn empty_and_blank_markdown_make_no_block() {
    assert!(parse("").is_empty());
    assert!(parse("  \n\n \t \n").is_empty());
}

// ---- out -----------------------------------------------------------------

#[test]
fn a_page_reads_back_as_markdown() {
    let doc = page_of(
        "# Kickoff\n\nSome **bold** text with [a link](https://acme.example).\n\n\
         - one\n- two\n\n1. first\n2. second\n\n- [ ] open\n- [x] done\n\n> quoted\n\n---\n\n\
         ```python\nprint(1)\n```",
    );
    let read = render(&doc, "Client onboarding", MAX_TEXT_BYTES);
    assert!(!read.truncated);
    assert_eq!(
        read.markdown,
        "# Client onboarding\n\n# Kickoff\n\n\
         Some **bold** text with [a link](https://acme.example).\n\n\
         - one\n- two\n\n1. first\n2. second\n\n- [ ] open\n- [x] done\n\n> quoted\n\n---\n\n\
         ```python\nprint(1)\n```"
    );
}

#[test]
fn mentions_and_embeds_are_references_never_resolved() {
    let doc = Doc::new();
    let task = "0192f0c4-7d4e-7b39-a7f1-3a8f0e2b9c11";
    let thread = "0192f0c4-7d4e-7b39-a7f1-3a8f0e2b9c22";
    let update = edit(&doc, |txn, fragment| {
        let line = fragment.push_back(txn, XmlElementPrelim::empty("paragraph"));
        line.push_back(txn, XmlTextPrelim::new("Send the pack, see "));
        let mention = line.push_back(txn, XmlElementPrelim::empty("mention"));
        mention.insert_attribute(txn, "ref_type", s("tasks.task"));
        mention.insert_attribute(txn, "ref_id", s(task));
        let person = line.push_back(txn, XmlElementPrelim::empty("mention"));
        person.insert_attribute(txn, "ref_type", s("person"));
        person.insert_attribute(txn, "ref_id", s("jonas@example.com"));
        let embed = fragment.push_back(txn, XmlElementPrelim::empty("embed"));
        embed.insert_attribute(txn, "ref_type", s("conversations.conversation"));
        embed.insert_attribute(txn, "ref_id", s(thread));
        let file = fragment.push_back(txn, XmlElementPrelim::empty("file"));
        file.insert_attribute(txn, "blob", s(&"a".repeat(64)));
        file.insert_attribute(txn, "name", s("Offer.pdf"));
        file.insert_attribute(txn, "size", Any::Number(12.0));
    });
    document::apply(&doc, &update).unwrap();
    let read = render(&doc, "", MAX_TEXT_BYTES);
    assert_eq!(
        read.markdown,
        format!(
            "Send the pack, see [tasks.task:{task}][person:jonas@example.com]\n\n\
             [conversations.conversation:{thread}]\n\n[file: Offer.pdf]"
        )
    );
}

#[test]
fn a_long_page_is_cut_after_the_last_block_that_fits() {
    let paragraph = "word ".repeat(40);
    let markdown = (0..40)
        .map(|n| format!("{n} {paragraph}"))
        .collect::<Vec<_>>()
        .join("\n\n");
    let doc = page_of(&markdown);
    let read = render(&doc, "Long", 1024);
    assert!(read.truncated);
    assert!(read.markdown.len() <= 1024);
    // Whole blocks only: every block that is there ends with its last word.
    for block in read.markdown.split("\n\n").skip(1) {
        assert!(block.ends_with("word"), "{block:?}");
    }
    let whole = render(&doc, "Long", MAX_TEXT_BYTES);
    assert!(!whole.truncated);
    assert!(whole.markdown.starts_with(&read.markdown));
}

#[test]
fn one_block_over_the_cap_still_gives_its_start() {
    let doc = page_of(&"x".repeat(5000));
    let read = render(&doc, "", 100);
    assert!(read.truncated);
    assert_eq!(read.markdown.len(), 100);
}

#[test]
fn what_cannot_be_read_is_left_out() {
    let blocks = [Block::paragraph(uuid::Uuid::now_v7(), "ok")];
    let doc = Doc::new();
    let added = append::append(
        &doc,
        &blocks,
        &[RecordRef::new("pages.page", &uuid::Uuid::nil())],
    )
    .unwrap();
    document::apply(&doc, &added.update).unwrap();
    assert_eq!(
        render(&doc, "", 1000).markdown,
        "ok\n\n[pages.page:00000000-0000-0000-0000-000000000000]"
    );
}

/// How many steps reading `markdown` may take: a constant per character of
/// it, however it is built. Each level of nesting reads its text once, finds
/// its brackets once, and searches at most once to the end for each of the
/// six delimiters before it remembers the failure; a found closer, a link's
/// address and a code span add at most one more look at each character. So
/// ten looks per level, and `MAX_DEPTH + 1` levels. A scan that started over
/// from each bracket would need the square of the length instead.
fn steps_allowed(markdown: &str) -> usize {
    10 * (MAX_DEPTH + 1) * markdown.chars().count()
}

/// Emphasis nested as deep as it goes, with openers that never close at
/// every level, so every level searches to the end for each delimiter.
fn nested_failing_openers() -> String {
    let failing = "`x ~~y *v [t]( ".repeat(150);
    let levels = [
        ("~~", "~~"),
        ("**", "**"),
        ("*", "*"),
        ("__", "__"),
        ("_", "_"),
        ("[", "](https://example.com)"),
    ];
    let mut text = failing.clone();
    for (open, close) in levels.iter().rev() {
        text = format!("{open}a {failing}{text} b{close}");
    }
    text
}

#[test]
fn brackets_and_emphasis_are_read_in_steps_proportional_to_the_text() {
    let nested = format!("{}x{}", "[".repeat(7900), "]".repeat(7900));
    let half = format!("{}x{}", "[".repeat(3900), "]".repeat(3900));
    let inside_emphasis = format!("**{}**", "[x".repeat(4000));
    let cases = [
        // Nested brackets, each matching one far away.
        nested.clone(),
        format!("*{nested}*"),
        format!("_{half}_ **{half}**"),
        inside_emphasis,
        // Many `[x]` with no `(` after them.
        "[x]".repeat(5000),
        "[x] ".repeat(4000),
        // Many `[a](` with no `)`, and with a `)` far away.
        "[a](".repeat(4000),
        format!("{}{})", "[a](".repeat(3000), "z".repeat(3000)),
        // An address that goes on and on.
        format!("[a]({})", "u".repeat(16 * 1024 - 8)),
        // Openers that never close, and closers that never match.
        "_a ".repeat(5000),
        "*a ".repeat(5000),
        "~~a ".repeat(4000),
        "_a_b".repeat(4000),
        "`a ".repeat(5000),
        // Failing openers at every level of nesting.
        nested_failing_openers(),
    ];
    for markdown in cases {
        assert!(markdown.len() <= 16 * 1024, "{}", markdown.len());
        let (blocks, steps) = parse_counted(&markdown);
        assert!(
            steps <= steps_allowed(&markdown),
            "{steps} steps for {} characters: {:.40}",
            markdown.chars().count(),
            markdown
        );
        // And whatever was made is still a page the schema accepts.
        let doc = Doc::new();
        let added = append::append(&doc, &blocks, &[]).unwrap();
        document::apply(&doc, &added.update).unwrap();
        document::inspect(&doc).unwrap();
    }
}

#[test]
fn nested_links_and_balanced_parentheses_still_read() {
    let blocks = parse("[one [two]](https://a.example/x_(y)) and [no](notalink");
    assert_eq!(text(&blocks[0]), "one [two] and [no](notalink");
    assert_eq!(
        blocks[0].runs[0].link.as_deref(),
        Some("https://a.example/x_(y)")
    );
    // An address too long to keep leaves the words.
    let long = format!("[words](https://a.example/{})", "u".repeat(3000));
    let blocks = parse(&long);
    assert_eq!(text(&blocks[0]), "words");
    assert!(blocks[0].runs.iter().all(|run| run.link.is_none()));
}
