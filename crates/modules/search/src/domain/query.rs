//! The query language: free words, `"phrases"`, `-word`, `a OR b` and
//! Gmail-style operators, parsed into a [`Query`]. Pure and total: any input
//! parses, unknown operators and bad values become text plus a
//! [`Warning`], never an error, so a half-typed query still answers. Only
//! the size limits ([`Limits`]) refuse a query.

use chrono::NaiveDate;
use serde::Serialize;
use utoipa::ToSchema;

/// The largest query, in characters.
pub const MAX_CHARS: usize = 512;
/// The most text terms.
pub const MAX_TERMS: usize = 16;
/// The most operators.
pub const MAX_OPERATORS: usize = 8;

/// A parsed query.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Query {
    /// Text clauses, all required; each matches if any of its terms does.
    pub clauses: Vec<Clause>,
    /// Filters, all required.
    pub filters: Vec<Filter>,
    /// What was not understood.
    pub warnings: Vec<Warning>,
}

/// Terms of which one must match (`a OR b`); a single term is the usual case.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Clause {
    /// The alternatives.
    pub any: Vec<Term>,
    /// The clause must *not* match (`-word`, `-"a phrase"`).
    pub negated: bool,
}

/// One text term.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Term {
    /// A word (or a token like `F-2026-0457`), stemmed in every language.
    Word(String),
    /// Words in this order.
    Phrase(String),
    /// The start of a word: the last word while the member is still typing.
    /// Letters and digits only.
    Prefix(String),
}

/// One filter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Filter {
    /// A person on an entry, partial and accent-insensitive.
    Person(PersonField, String),
    /// Part of the group's title.
    Subject(String),
    /// An entry with files.
    HasAttachment,
    /// Where entries sit.
    Mailbox(Mailbox),
    /// The group's stream.
    Stream(String),
    /// A group flag.
    Is(Flag),
    /// A label, by name.
    Label(String),
    /// Entries on or after this day (in the caller's time zone).
    After(NaiveDate),
    /// Entries before this day.
    Before(NaiveDate),
    /// Entries newer than this many days.
    NewerThanDays(u32),
    /// Entries older than this many days.
    OlderThanDays(u32),
}

/// `from:`, `to:` or `cc:`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum PersonField {
    /// The author.
    From,
    /// A recipient.
    To,
    /// A copied recipient.
    Cc,
}

/// `in:inbox|archive|spam|trash|anywhere`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mailbox {
    /// In the inbox.
    Inbox,
    /// Out of the inbox, not in spam or trash.
    Archive,
    /// In spam.
    Spam,
    /// In the trash.
    Trash,
    /// Anywhere, spam and trash included.
    Anywhere,
}

/// `is:unread|read|starred`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flag {
    /// Something unread.
    Unread,
    /// Nothing unread.
    Read,
    /// Starred.
    Starred,
}

/// Something in the query that was not understood.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct Warning {
    /// `unknown_operator`, `empty_value` or `invalid_value`.
    pub code: &'static str,
    /// The operator, as typed (lower-cased).
    pub operator: String,
}

/// Why a query is refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TooLong {
    /// More than [`MAX_CHARS`] characters.
    Chars,
    /// More than [`MAX_TERMS`] terms.
    Terms,
    /// More than [`MAX_OPERATORS`] operators.
    Operators,
}

/// A piece of the input.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Token {
    Word { text: String, negated: bool },
    Phrase { text: String, negated: bool },
    Operator { name: String, value: String },
    Or,
}

/// Parse `input`. With `prefix`, the last free word (if it is the end of the
/// input, two or more letters or digits) matches as a prefix.
pub fn parse(input: &str, prefix: bool) -> Result<Query, TooLong> {
    if input.chars().count() > MAX_CHARS {
        return Err(TooLong::Chars);
    }
    let tokens = tokenize(input);
    let operators = tokens
        .iter()
        .filter(|t| matches!(t, Token::Operator { .. }))
        .count();
    if operators > MAX_OPERATORS {
        return Err(TooLong::Operators);
    }
    let mut query = Query::default();
    let mut terms: Vec<(Term, bool)> = Vec::new();
    let mut joins_previous = false;
    let ends_in_word = !input.ends_with(char::is_whitespace);
    let last_word = tokens
        .iter()
        .rposition(|t| matches!(t, Token::Word { negated: false, .. }));
    let typed_last = tokens.len().checked_sub(1);
    let mut clauses: Vec<Clause> = Vec::new();
    for (index, token) in tokens.into_iter().enumerate() {
        let term = match token {
            Token::Or => {
                joins_previous = !clauses.is_empty();
                continue;
            }
            Token::Operator { name, value } => {
                if let Some(text) = operator(&name, &value, &mut query) {
                    Some((Term::Word(text), false))
                } else {
                    None
                }
            }
            Token::Word { text, negated } => {
                let is_prefix = prefix
                    && ends_in_word
                    && !negated
                    && Some(index) == last_word
                    && Some(index) == typed_last
                    && text.chars().count() >= 2
                    && text.chars().all(char::is_alphanumeric);
                Some((
                    if is_prefix {
                        Term::Prefix(text)
                    } else {
                        Term::Word(text)
                    },
                    negated,
                ))
            }
            Token::Phrase { text, negated } => Some((Term::Phrase(text), negated)),
        };
        let Some((term, negated)) = term else {
            joins_previous = false;
            continue;
        };
        terms.push((term.clone(), negated));
        match clauses.last_mut() {
            Some(last) if joins_previous && !negated && !last.negated => last.any.push(term),
            _ => clauses.push(Clause {
                any: vec![term],
                negated,
            }),
        }
        joins_previous = false;
    }
    if terms.len() > MAX_TERMS {
        return Err(TooLong::Terms);
    }
    query.clauses = clauses;
    Ok(query)
}

/// Apply operator `name:value` to `query`; returns text to search instead
/// when it is not an operator we know or its value is not valid.
fn operator(name: &str, value: &str, query: &mut Query) -> Option<String> {
    let warn = |query: &mut Query, code| {
        query.warnings.push(Warning {
            code,
            operator: name.to_owned(),
        });
    };
    let known = [
        "from",
        "to",
        "cc",
        "subject",
        "has",
        "in",
        "is",
        "label",
        "before",
        "after",
        "older_than",
        "newer_than",
    ];
    if !known.contains(&name) {
        warn(query, "unknown_operator");
        return Some(format!("{name}:{value}"));
    }
    let value = value.trim();
    if value.is_empty() {
        warn(query, "empty_value");
        return None;
    }
    let lower = value.to_lowercase();
    let filter = match name {
        "from" => Some(Filter::Person(PersonField::From, value.to_owned())),
        "to" => Some(Filter::Person(PersonField::To, value.to_owned())),
        "cc" => Some(Filter::Person(PersonField::Cc, value.to_owned())),
        "subject" => Some(Filter::Subject(value.to_owned())),
        "label" => Some(Filter::Label(value.to_owned())),
        "has" => (["attachment", "attachments", "file", "files"].contains(&lower.as_str()))
            .then_some(Filter::HasAttachment),
        "in" => Some(match lower.as_str() {
            "inbox" => Filter::Mailbox(Mailbox::Inbox),
            "archive" | "archived" => Filter::Mailbox(Mailbox::Archive),
            "spam" => Filter::Mailbox(Mailbox::Spam),
            "trash" => Filter::Mailbox(Mailbox::Trash),
            "anywhere" | "all" => Filter::Mailbox(Mailbox::Anywhere),
            stream
                if stream
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_') =>
            {
                Filter::Stream(stream.to_owned())
            }
            _ => {
                warn(query, "invalid_value");
                return Some(value.to_owned());
            }
        }),
        "is" => match lower.as_str() {
            "unread" => Some(Filter::Is(Flag::Unread)),
            "read" => Some(Filter::Is(Flag::Read)),
            "starred" => Some(Filter::Is(Flag::Starred)),
            _ => None,
        },
        "before" => date(value).map(Filter::Before),
        "after" => date(value).map(Filter::After),
        "older_than" => days(&lower).map(Filter::OlderThanDays),
        "newer_than" => days(&lower).map(Filter::NewerThanDays),
        _ => None,
    };
    if let Some(filter) = filter {
        query.filters.push(filter);
        None
    } else {
        warn(query, "invalid_value");
        Some(value.to_owned())
    }
}

/// `2026-10-01` or `2026/10/01`.
fn date(value: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(&value.replace('/', "-"), "%Y-%m-%d").ok()
}

/// `7d`, `2w`, `3m`, `1y` in days (a month is 30 days, a year 365).
fn days(value: &str) -> Option<u32> {
    let (number, unit) = value.split_at(value.len().checked_sub(1)?);
    let number: u32 = number.parse().ok()?;
    let per = match unit {
        "d" => 1,
        "w" => 7,
        "m" => 30,
        "y" => 365,
        _ => return None,
    };
    number.checked_mul(per).filter(|days| *days <= 365 * 100)
}

/// Split `input` into words, phrases, operators and `OR`.
fn tokenize(input: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut chars = input.chars().peekable();
    while let Some(&c) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
            continue;
        }
        let negated = c == '-' && {
            chars.next();
            chars.peek().is_some_and(|next| !next.is_whitespace())
        };
        if chars.peek() == Some(&'"') {
            chars.next();
            let text: String = chars.by_ref().take_while(|&c| c != '"').collect();
            let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
            if !text.is_empty() {
                tokens.push(Token::Phrase { text, negated });
            }
            continue;
        }
        let mut word = String::new();
        let mut quoted_value = None;
        while let Some(&c) = chars.peek() {
            if c.is_whitespace() {
                break;
            }
            chars.next();
            if c == ':' && !word.contains(':') && chars.peek() == Some(&'"') {
                chars.next();
                quoted_value = Some(chars.by_ref().take_while(|&c| c != '"').collect::<String>());
                word.push(':');
                break;
            }
            word.push(c);
        }
        if word.is_empty() {
            // A lone "-".
            continue;
        }
        if word == "OR" && !negated {
            tokens.push(Token::Or);
            continue;
        }
        match word.split_once(':') {
            Some((name, value))
                if !negated
                    && !name.is_empty()
                    && name
                        .chars()
                        .all(|c| c.is_ascii_alphabetic() || c == '_') =>
            {
                tokens.push(Token::Operator {
                    name: name.to_ascii_lowercase(),
                    value: quoted_value.unwrap_or_else(|| value.to_owned()),
                });
            }
            _ => {
                let text = match quoted_value {
                    Some(value) => format!("{word}{value}"),
                    None => word,
                };
                tokens.push(Token::Word { text, negated });
            }
        }
    }
    tokens
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(query: &Query) -> Vec<String> {
        query
            .clauses
            .iter()
            .flat_map(|c| c.any.iter())
            .map(|t| match t {
                Term::Word(w) | Term::Phrase(w) | Term::Prefix(w) => w.clone(),
            })
            .collect()
    }

    #[test]
    fn words_phrases_and_negation() {
        let query = parse(r#"invoice "board meeting" -draft"#, false).unwrap();
        assert_eq!(
            query.clauses,
            vec![
                Clause {
                    any: vec![Term::Word("invoice".into())],
                    negated: false
                },
                Clause {
                    any: vec![Term::Phrase("board meeting".into())],
                    negated: false
                },
                Clause {
                    any: vec![Term::Word("draft".into())],
                    negated: true
                },
            ]
        );
        assert!(query.filters.is_empty() && query.warnings.is_empty());
    }

    #[test]
    fn or_joins_neighbours() {
        let query = parse("fatura OR factura recibo", false).unwrap();
        assert_eq!(query.clauses.len(), 2);
        assert_eq!(query.clauses[0].any.len(), 2);
        assert_eq!(words(&query), ["fatura", "factura", "recibo"]);
        // A dangling OR is ignored.
        assert_eq!(parse("OR fatura OR", false).unwrap().clauses.len(), 1);
    }

    #[test]
    fn operators_become_filters() {
        let query = parse(
            r#"from:ana to:"Sam Rivera" has:attachment is:unread in:receipts in:trash label:Clients after:2026/09/01 before:2026-10-01 newer_than:2w older_than:1y subject:plan"#,
            false,
        );
        // Twelve operators: over the limit.
        assert_eq!(query, Err(TooLong::Operators));
        let query = parse(
            r#"from:ana to:"Sam Rivera" has:attachment is:unread in:receipts label:Clients after:2026/09/01 newer_than:2w invoice"#,
            false,
        )
        .unwrap();
        assert_eq!(
            query.filters,
            vec![
                Filter::Person(PersonField::From, "ana".into()),
                Filter::Person(PersonField::To, "Sam Rivera".into()),
                Filter::HasAttachment,
                Filter::Is(Flag::Unread),
                Filter::Stream("receipts".into()),
                Filter::Label("Clients".into()),
                Filter::After(NaiveDate::from_ymd_opt(2026, 9, 1).unwrap()),
                Filter::NewerThanDays(14),
            ]
        );
        assert_eq!(words(&query), ["invoice"]);
        assert!(query.warnings.is_empty());
    }

    #[test]
    fn half_typed_and_unknown_operators_warn() {
        let query = parse("invoice from:", false).unwrap();
        assert_eq!(words(&query), ["invoice"]);
        assert_eq!(
            query.warnings,
            vec![Warning {
                code: "empty_value",
                operator: "from".into()
            }]
        );
        let query = parse("colour:red is:maybe before:soon", false).unwrap();
        assert_eq!(words(&query), ["colour:red", "maybe", "soon"]);
        let codes: Vec<_> = query.warnings.iter().map(|w| w.code).collect();
        assert_eq!(codes, ["unknown_operator", "invalid_value", "invalid_value"]);
        assert!(query.filters.is_empty());
    }

    #[test]
    fn urls_and_codes_stay_words() {
        let query = parse("F-2026-0457 https://example.com/x 23,47", false).unwrap();
        // `https:` is not an operator we know: kept whole, with a warning.
        assert_eq!(
            words(&query),
            ["F-2026-0457", "https://example.com/x", "23,47"]
        );
    }

    #[test]
    fn the_last_word_is_a_prefix_while_typing() {
        let query = parse("boiler rep", true).unwrap();
        assert_eq!(
            query.clauses[1].any,
            vec![Term::Prefix("rep".into())],
            "{query:?}"
        );
        assert_eq!(query.clauses[0].any, vec![Term::Word("boiler".into())]);
        // Not after a space, not for one letter, not for punctuation.
        assert!(matches!(
            parse("boiler rep ", true).unwrap().clauses[1].any[0],
            Term::Word(_)
        ));
        assert!(matches!(
            parse("boiler r", true).unwrap().clauses[1].any[0],
            Term::Word(_)
        ));
        assert!(matches!(
            parse("F-20", true).unwrap().clauses[0].any[0],
            Term::Word(_)
        ));
        assert!(matches!(
            parse("boiler rep", false).unwrap().clauses[1].any[0],
            Term::Word(_)
        ));
        assert!(matches!(
            parse("joão", true).unwrap().clauses[0].any[0],
            Term::Prefix(_)
        ));
    }

    #[test]
    fn limits() {
        assert_eq!(parse(&"a".repeat(513), false), Err(TooLong::Chars));
        assert!(parse(&"a".repeat(512), false).is_ok());
        let seventeen = vec!["w"; 17].join(" ");
        assert_eq!(parse(&seventeen, false), Err(TooLong::Terms));
        let nine = vec!["is:unread"; 9].join(" ");
        assert_eq!(parse(&nine, false), Err(TooLong::Operators));
    }

    #[test]
    fn any_input_parses() {
        for input in [
            "", " ", "-", "--", "\"", "\"\"", ":", "a:", ":b", "-\"x", "OR", "from:\"", "é:ç",
            "\u{e000}x\u{e001}", "a\"b\"c", "-from:ana",
        ] {
            assert!(parse(input, true).is_ok(), "{input:?}");
        }
        // `-from:ana` is a negated word, not a filter.
        assert!(parse("-from:ana", false).unwrap().filters.is_empty());
    }
}
