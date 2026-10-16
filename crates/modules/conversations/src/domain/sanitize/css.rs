//! CSS from untrusted mail, read with a real CSS parser (cssparser, the
//! tokenizer of Servo and Firefox's style engine): what survives can load
//! nothing and run nothing, whatever the property.
//!
//! A declaration is kept only when every token of its value is inert:
//! no `url(`, no function outside [`FUNCTIONS`] (so no `image-set(`,
//! `src(`, `expression(`, `var(`, `attr(`, `env(`), no at-keyword, no
//! malformed URL or string, no CSS escape (`\`) and no `<`. Custom
//! properties, the scripting properties of old engines (`behavior`,
//! `-moz-binding`) and lengths in viewport units are dropped too (the
//! reader sizes the frame to its content, and a length that follows the
//! frame would grow it without end).

use cssparser::{
    AtRuleParser, CowRcStr, DeclarationParser, ParseError, Parser, ParserState,
    QualifiedRuleParser, RuleBodyItemParser, RuleBodyParser, StyleSheetParser, Token,
};

/// Functions a kept value may call: colours, maths, gradients, transforms,
/// filters, timing, shapes, counters and grid. None of them loads anything.
const FUNCTIONS: &[&str] = &[
    "rgb",
    "rgba",
    "hsl",
    "hsla",
    "hwb",
    "lab",
    "lch",
    "oklab",
    "oklch",
    "color",
    "color-mix",
    "calc",
    "min",
    "max",
    "clamp",
    "linear-gradient",
    "radial-gradient",
    "conic-gradient",
    "repeating-linear-gradient",
    "repeating-radial-gradient",
    "repeating-conic-gradient",
    "-webkit-linear-gradient",
    "-webkit-radial-gradient",
    "translate",
    "translatex",
    "translatey",
    "translate3d",
    "scale",
    "scalex",
    "scaley",
    "rotate",
    "skew",
    "skewx",
    "skewy",
    "matrix",
    "perspective",
    "blur",
    "brightness",
    "contrast",
    "drop-shadow",
    "grayscale",
    "hue-rotate",
    "invert",
    "opacity",
    "saturate",
    "sepia",
    "cubic-bezier",
    "steps",
    "rect",
    "inset",
    "circle",
    "ellipse",
    "polygon",
    "counter",
    "counters",
    "minmax",
    "repeat",
    "fit-content",
];

/// Pseudo-class functions a kept selector may use.
const SELECTOR_FUNCTIONS: &[&str] = &[
    "not",
    "is",
    "where",
    "has",
    "nth-child",
    "nth-last-child",
    "nth-of-type",
    "nth-last-of-type",
    "lang",
    "dir",
];

/// Properties never kept, whatever their value: they ran script in old
/// engines.
const PROPERTIES: [&str; 3] = ["behavior", "-moz-binding", "-ms-behavior"];

/// Deepest nesting of blocks and functions read; anything deeper is dropped.
const MAX_DEPTH: usize = 8;

/// Most bytes of `@media` rules handed to the client.
pub const MAX_MEDIA_BYTES: usize = 64 * 1024;

/// One kept declaration: the property, lower-case, and its value as written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Declaration {
    pub name: String,
    pub value: String,
}

/// The declarations of a declaration block (a `style` attribute, or a rule's
/// body) that load and run nothing.
pub fn declarations(block: &str) -> Vec<Declaration> {
    let mut parser = Parser::new(block);
    let mut declarations = Declarations;
    RuleBodyParser::new(&mut parser, &mut declarations)
        .filter_map(Result::ok)
        .collect()
}

/// [`declarations`] written back as a `style` attribute, or `None` when
/// none are kept.
pub fn style_attribute(style: &str) -> Option<String> {
    let kept = declarations(style);
    (!kept.is_empty()).then(|| join(&kept))
}

fn join(declarations: &[Declaration]) -> String {
    declarations
        .iter()
        .map(|d| format!("{}: {}", d.name, d.value))
        .collect::<Vec<_>>()
        .join("; ")
}

/// The `@media` rules of `sheet` that load and run nothing, for the client
/// to apply to the frame's document only: each keeps its style rules with
/// their kept declarations. Every other rule is left out (plain style rules
/// are inlined into `style` attributes instead), and so is any `@media`
/// about `prefers-color-scheme` (mail is shown on a white page, as written
/// for light). `None` when nothing is kept or the result would pass
/// [`MAX_MEDIA_BYTES`].
pub fn media_rules(sheet: &str) -> Option<String> {
    let mut parser = Parser::new(sheet);
    let mut top = TopLevel;
    let rules: Vec<String> = StyleSheetParser::new(&mut parser, &mut top)
        .filter_map(Result::ok)
        .flatten()
        .collect();
    let css = rules.join("\n");
    (!css.is_empty() && css.len() <= MAX_MEDIA_BYTES).then_some(css)
}

/// Whether the source text could hide what it does (escapes) or end an
/// HTML context it is put in (`<`).
fn suspicious(text: &str) -> bool {
    text.contains('\\') || text.contains('<')
}

/// A unit that measures the viewport.
fn viewport_unit(unit: &str) -> bool {
    let unit = unit.to_ascii_lowercase();
    unit.ends_with("vh")
        || unit.ends_with("vw")
        || unit.ends_with("vb")
        || unit.ends_with("vi")
        || unit.ends_with("vmin")
        || unit.ends_with("vmax")
}

/// Whether every token left in `input` is inert for a declaration value.
fn inert_value(input: &mut Parser<'_>, depth: usize) -> bool {
    if depth > MAX_DEPTH {
        return false;
    }
    loop {
        let token = match input.next_including_whitespace_and_comments() {
            Ok(token) => token.clone(),
            Err(_) => return true,
        };
        let nested = match &token {
            Token::UnquotedUrl(_)
            | Token::BadUrl(_)
            | Token::BadString(_)
            | Token::AtKeyword(_)
            | Token::CDO
            | Token::CDC => return false,
            Token::Delim(c) if *c == '\\' || *c == '<' => return false,
            Token::Dimension { unit, .. } if viewport_unit(unit) => return false,
            Token::Function(name) => {
                let name = name.to_ascii_lowercase();
                if !FUNCTIONS.contains(&name.as_str()) {
                    return false;
                }
                true
            }
            Token::ParenthesisBlock | Token::SquareBracketBlock | Token::CurlyBracketBlock => true,
            _ => false,
        };
        if nested
            && !input
                .parse_nested_block(|inner| {
                    if inert_value(inner, depth + 1) {
                        Ok(())
                    } else {
                        Err(ParseError::<()>::custom(()))
                    }
                })
                .is_ok()
        {
            return false;
        }
    }
}

/// Whether every token left in `input` is inert for a selector or a media
/// query: no URLs, at-keywords, blocks of declarations, malformed tokens,
/// and functions only from `functions`.
fn inert_prelude(input: &mut Parser<'_>, functions: &[&str], depth: usize) -> bool {
    if depth > MAX_DEPTH {
        return false;
    }
    loop {
        let token = match input.next_including_whitespace_and_comments() {
            Ok(token) => token.clone(),
            Err(_) => return true,
        };
        let nested = match &token {
            Token::UnquotedUrl(_)
            | Token::BadUrl(_)
            | Token::BadString(_)
            | Token::AtKeyword(_)
            | Token::CurlyBracketBlock
            | Token::Semicolon
            | Token::CDO
            | Token::CDC => return false,
            Token::Delim(c) if *c == '\\' || *c == '<' => return false,
            Token::Function(name) => {
                let name = name.to_ascii_lowercase();
                if !functions.contains(&name.as_str()) {
                    return false;
                }
                true
            }
            Token::ParenthesisBlock | Token::SquareBracketBlock => true,
            _ => false,
        };
        if nested
            && !input
                .parse_nested_block(|inner| {
                    if inert_prelude(inner, functions, depth + 1) {
                        Ok(())
                    } else {
                        Err(ParseError::<()>::custom(()))
                    }
                })
                .is_ok()
        {
            return false;
        }
    }
}

/// Reads declarations, keeping the inert ones.
struct Declarations;

impl<'i> DeclarationParser<'i> for Declarations {
    type Declaration = Declaration;
    type Error = ();

    fn parse_value(
        &mut self,
        name: CowRcStr<'i>,
        input: &mut Parser<'i>,
        _start: &ParserState,
    ) -> Result<Declaration, ParseError<()>> {
        let name = name.to_ascii_lowercase();
        let start = input.position();
        let inert = inert_value(input, 0);
        let value = input.slice_from(start).trim();
        if !inert
            || value.is_empty()
            || name.starts_with("--")
            || PROPERTIES.contains(&name.as_str())
            || suspicious(&name)
            || suspicious(value)
        {
            return Err(ParseError::custom(()));
        }
        Ok(Declaration {
            name,
            value: value.to_owned(),
        })
    }
}

impl<'i> AtRuleParser<'i> for Declarations {
    type Prelude = ();
    type AtRule = Declaration;
    type Error = ();
}

impl<'i> QualifiedRuleParser<'i> for Declarations {
    type Prelude = ();
    type QualifiedRule = Declaration;
    type Error = ();
}

impl<'i> RuleBodyItemParser<'i, Declaration, ()> for Declarations {
    fn parse_declarations(&self) -> bool {
        true
    }
    fn parse_qualified(&self) -> bool {
        false
    }
}

/// Reads a style sheet's top level: only `@media` rules give anything.
struct TopLevel;

impl<'i> AtRuleParser<'i> for TopLevel {
    type Prelude = String;
    type AtRule = Option<String>;
    type Error = ();

    fn parse_prelude(
        &mut self,
        name: CowRcStr<'i>,
        input: &mut Parser<'i>,
    ) -> Result<String, ParseError<()>> {
        if !name.eq_ignore_ascii_case("media") {
            return Err(ParseError::custom(()));
        }
        let start = input.position();
        let inert = inert_prelude(input, &[], 0);
        let prelude = input.slice_from(start).trim();
        if !inert
            || suspicious(prelude)
            || prelude
                .to_ascii_lowercase()
                .contains("prefers-color-scheme")
        {
            return Err(ParseError::custom(()));
        }
        Ok(prelude.to_owned())
    }

    fn parse_block(
        &mut self,
        prelude: String,
        _start: &ParserState,
        input: &mut Parser<'i>,
    ) -> Result<Option<String>, ParseError<()>> {
        let mut rules = StyleRules;
        let kept: Vec<String> = RuleBodyParser::new(input, &mut rules)
            .filter_map(Result::ok)
            .collect();
        Ok((!kept.is_empty()).then(|| format!("@media {prelude} {{\n{}\n}}", kept.join("\n"))))
    }
}

// Plain style rules are inlined, not handed on: the default skips them.
impl<'i> QualifiedRuleParser<'i> for TopLevel {
    type Prelude = ();
    type QualifiedRule = Option<String>;
    type Error = ();
}

/// Reads the style rules inside a `@media` block.
struct StyleRules;

impl<'i> QualifiedRuleParser<'i> for StyleRules {
    type Prelude = String;
    type QualifiedRule = String;
    type Error = ();

    fn parse_prelude(&mut self, input: &mut Parser<'i>) -> Result<String, ParseError<()>> {
        let start = input.position();
        let inert = inert_prelude(input, SELECTOR_FUNCTIONS, 0);
        let selector = input.slice_from(start).trim();
        if !inert || selector.is_empty() || suspicious(selector) {
            return Err(ParseError::custom(()));
        }
        Ok(selector.to_owned())
    }

    fn parse_block(
        &mut self,
        selector: String,
        _start: &ParserState,
        input: &mut Parser<'i>,
    ) -> Result<String, ParseError<()>> {
        let mut declarations = Declarations;
        let kept: Vec<Declaration> = RuleBodyParser::new(input, &mut declarations)
            .filter_map(Result::ok)
            .collect();
        if kept.is_empty() {
            return Err(ParseError::custom(()));
        }
        Ok(format!("{selector} {{ {} }}", join(&kept)))
    }
}

impl<'i> AtRuleParser<'i> for StyleRules {
    type Prelude = ();
    type AtRule = String;
    type Error = ();
}

impl<'i> DeclarationParser<'i> for StyleRules {
    type Declaration = String;
    type Error = ();
}

impl<'i> RuleBodyItemParser<'i, String, ()> for StyleRules {
    fn parse_declarations(&self) -> bool {
        false
    }
    fn parse_qualified(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kept(style: &str) -> Option<String> {
        style_attribute(style)
    }

    #[test]
    fn plain_declarations_are_kept_as_written() {
        assert_eq!(
            kept("color: #333; font: bold 16px/1.4 \"Segoe UI\", Arial; padding:0 8px !important")
                .as_deref(),
            Some("color: #333; font: bold 16px/1.4 \"Segoe UI\", Arial; padding: 0 8px !important")
        );
        assert_eq!(
            kept("background: linear-gradient(90deg, rgb(0 0 0 / 50%), #fff); width: calc(100% - 2px)").as_deref(),
            Some("background: linear-gradient(90deg, rgb(0 0 0 / 50%), #fff); width: calc(100% - 2px)")
        );
    }

    #[test]
    fn anything_that_loads_or_runs_is_dropped() {
        for style in [
            "background: url(https://t.example/a.gif)",
            "background: URL( 'https://t.example/a.gif' )",
            "background-image: url(\"https://t.example/a.gif\")",
            "background: u\\72l(https://t.example/a.gif)",
            "background: \\75 rl(https://t.example/a.gif)",
            "background-image: image-set(\"https://t.example/a.png\" 1x)",
            "background-image: -webkit-image-set(url(a.png) 1x)",
            "background: cross-fade(url(a.png), url(b.png), 50%)",
            "background: src(\"https://t.example/a.png\")",
            "width: expression(alert(1))",
            "behavior: url(x.htc)",
            "-moz-binding: url(https://t.example/x.xml#x)",
            "--beacon: url(https://t.example/a.gif)",
            "background: var(--beacon)",
            "content: attr(data-x url)",
            "list-style: symbols(cyclic url(a.png))",
            "background: paint(beacon)",
            "background: element(#x)",
            "background: rgb(calc(url(a)))",
            "content: '</style><script>'",
            "height: 100vh",
            "min-height: 50dvh",
            "width: 10vmax",
            "font-family: @import",
        ] {
            assert_eq!(kept(style), None, "{style}");
        }
    }

    #[test]
    fn a_bad_declaration_does_not_take_its_neighbours_with_it() {
        assert_eq!(
            kept("color: red; background: url(https://t.example/a.gif); font-weight: bold")
                .as_deref(),
            Some("color: red; font-weight: bold")
        );
        assert_eq!(kept("}; color: red").as_deref(), Some("color: red"));
    }

    #[test]
    fn deep_nesting_is_dropped() {
        let deep = format!("width: {}1px{}", "calc(".repeat(40), ")".repeat(40));
        assert_eq!(kept(&deep), None);
    }

    #[test]
    fn media_rules_keep_only_inert_style_rules() {
        let sheet = r#"
            @import url("https://t.example/a.css");
            @font-face { font-family: Spy; src: url(https://t.example/f.woff2); }
            body { color: red; }
            @media only screen and (max-width: 620px) {
                table.body .container { width: 100% !important; padding: 0 !important; }
                .hero { background: url(https://t.example/hero.png); }
                .mixed { color: #111 !important; background-image: url(x); }
                @media (min-width: 1px) { .nested { color: blue; } }
                .bad\:x { color: red; }
            }
            @media (prefers-color-scheme: dark) { body { background: #000 !important; } }
            @media print { .no-print { display: none; } }
            @supports (display: grid) { .g { display: grid; } }
            @media screen and (max-width: 400px), url(x) { .x { color: red; } }
        "#;
        let css = media_rules(sheet).unwrap_or_default();
        assert_eq!(
            css,
            "@media only screen and (max-width: 620px) {\ntable.body .container { width: 100% !important; padding: 0 !important }\n.mixed { color: #111 !important }\n}\n@media print {\n.no-print { display: none }\n}"
        );
    }

    #[test]
    fn nothing_kept_is_none() {
        assert_eq!(media_rules("body { color: red }"), None);
        assert_eq!(
            media_rules("@media (max-width: 1px) { .a { background: url(x) } }"),
            None
        );
        assert_eq!(media_rules(""), None);
    }
}
