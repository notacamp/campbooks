//! Errors described without content, for logs and `kernel.jobs.last_error`.
//!
//! Some errors quote the data they failed on: a `serde_json` error names the
//! value it could not read (a subject, an address), and a Postgres error's
//! message and `DETAIL` can carry a row's values (`Key (email)=(…) already
//! exists`). What is logged or stored about a failure is its kind and where
//! it happened, never that data. These helpers say exactly that.

use std::error::Error as StdError;

/// What kind of JSON error, and where: never the offending text.
pub fn json(error: &serde_json::Error) -> String {
    format!(
        "{:?} error at line {}, column {}",
        error.classify(),
        error.line(),
        error.column()
    )
}

/// A database error by kind: for a server error its SQLSTATE, constraint and
/// table, never its message or `DETAIL`.
pub fn sqlx(error: &sqlx::Error) -> String {
    match error {
        sqlx::Error::Database(db) => {
            let code = db.code();
            let constraint = db
                .constraint()
                .map(|c| format!(" on constraint {c}"))
                .unwrap_or_default();
            let table = db
                .table()
                .map(|t| format!(" in table {t}"))
                .unwrap_or_default();
            format!(
                "database error {}{constraint}{table}",
                code.as_deref().unwrap_or("(no SQLSTATE)")
            )
        }
        sqlx::Error::RowNotFound => "no row found".to_owned(),
        sqlx::Error::PoolTimedOut => "database pool timed out".to_owned(),
        sqlx::Error::PoolClosed => "database pool closed".to_owned(),
        sqlx::Error::Io(io) => format!("database i/o error ({:?})", io.kind()),
        sqlx::Error::Tls(_) => "database TLS error".to_owned(),
        sqlx::Error::Protocol(_) => "database protocol error".to_owned(),
        sqlx::Error::ColumnDecode { index, .. } => format!("column {index} did not decode"),
        sqlx::Error::ColumnNotFound(name) => format!("column {name} not found"),
        sqlx::Error::TypeNotFound { type_name } => format!("type {type_name} not found"),
        sqlx::Error::Decode(_) => "a value did not decode".to_owned(),
        sqlx::Error::Encode(_) => "a value did not encode".to_owned(),
        sqlx::Error::Configuration(_) => "database configuration error".to_owned(),
        sqlx::Error::Migrate(_) => "database migration error".to_owned(),
        _ => "database error".to_owned(),
    }
}

/// Any error, safe to log: a database or JSON error anywhere in its chain is
/// described by kind ([`sqlx()`], [`json`]); anything else by its own message
/// only, never its `Debug` form or the messages of its sources.
pub fn error(error: &(dyn StdError + 'static)) -> String {
    let mut current: Option<&(dyn StdError + 'static)> = Some(error);
    while let Some(err) = current {
        if let Some(db) = err.downcast_ref::<sqlx::Error>() {
            return sqlx(db);
        }
        if let Some(json_error) = err.downcast_ref::<serde_json::Error>() {
            return json(json_error);
        }
        current = err.source();
    }
    error.to_string()
}

/// A job's failure message as `kernel.jobs.last_error` keeps it and the
/// worker logs it. Job handlers never put content in a
/// [`JobError`](crate::jobs::JobError); this is the backstop, applied when
/// the message is written, for the shapes content takes when it slips in
/// anyway (a provider's reply quoted into the message, an address, a link):
///
/// - only the first line is kept (bodies and replies span lines), followed
///   by ` […]` when more came after it;
/// - anything quoted (`"…"`, `'…'`, `` `…` ``, `“…”`, `‘…’`, `«…»`) becomes
///   `"…"`, and a quote that never closes hides the rest of the line;
/// - an email address becomes `[address]`;
/// - a URL keeps its scheme and host (`https://mail.example.com/…`), never
///   its user, path or query;
/// - an opaque run of [`OPAQUE_RUN`] or more letters, digits and `+/=_-.`
///   (a token, an encoded body) becomes `[…]`;
/// - other control characters become spaces.
///
/// The handler's own wording, ids, numbers and status codes stay, so an
/// administrator still sees what failed and where.
pub fn job_error(message: &str) -> String {
    let (line, more) = match message.find(['\n', '\r']) {
        Some(at) => (&message[..at], !message[at..].trim().is_empty()),
        None => (message, false),
    };
    let unquoted = mask_quotes(line);
    let mut out = String::with_capacity(unquoted.len());
    for (index, word) in unquoted.split(' ').enumerate() {
        if index > 0 {
            out.push(' ');
        }
        mask_word(word, &mut out);
    }
    if more {
        out.push_str(" […]");
    }
    out
}

/// Longest value [`looks_like_an_id`] accepts.
const MAX_ID_LEN: usize = 200;

/// Longest [`error_kind`].
pub const MAX_KIND_LEN: usize = 120;

/// What kind of error `text` describes, never what it was about: `text`
/// through [`job_error`] (quotes, addresses, URLs' paths and
/// tokens masked, first line only), then only its head, before the first
/// `: `, `; ` or ` (` where an error's message usually goes on to quote
/// what it failed on (`job failed: <the provider's reply>`), at most
/// [`MAX_KIND_LEN`] characters.
pub fn error_kind(text: &str) -> String {
    let redacted = job_error(text);
    let head = [": ", "; ", " ("]
        .iter()
        .filter_map(|separator| redacted.find(separator))
        .min()
        .map_or(redacted.as_str(), |at| &redacted[..at]);
    let head = head.trim();
    match head.char_indices().nth(MAX_KIND_LEN) {
        Some((at, _)) => format!("{}…", &head[..at]),
        None => head.to_owned(),
    }
}

/// Whether `value` reads like an id, a name or a route template rather than
/// data: short, only letters, digits and `-_.:/{}`, and no run of
/// [`OPAQUE_RUN`] characters between separators (a token).
pub fn looks_like_an_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_ID_LEN
        && value.chars().all(|c| {
            c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | ':' | '/' | '{' | '}')
        })
        && value
            .split(['/', '{', '}', ':'])
            .all(|segment| segment.len() < OPAQUE_RUN)
}

/// Shortest run of token-like characters [`job_error`] hides. Longer than a
/// UUID (36), so ids stay readable.
pub const OPAQUE_RUN: usize = 40;

/// The quote that closes one opened by `open`, when `open` opens quotes.
fn closing_quote(open: char) -> Option<char> {
    match open {
        '"' | '\'' | '`' => Some(open),
        '“' => Some('”'),
        '‘' => Some('’'),
        '«' => Some('»'),
        _ => None,
    }
}

/// `text` with every quoted span replaced by `"…"` and control characters by
/// spaces. An apostrophe inside or at the end of a word (`batch's`,
/// `users'`) is not a quote.
fn mask_quotes(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut previous: Option<char> = None;
    let mut rest = text;
    while let Some(c) = rest.chars().next() {
        rest = &rest[c.len_utf8()..];
        let opens =
            closing_quote(c).filter(|_| c != '\'' || !previous.is_some_and(char::is_alphanumeric));
        let Some(close) = opens else {
            let c = if c.is_control() { ' ' } else { c };
            out.push(c);
            previous = Some(c);
            continue;
        };
        out.push_str("\"…");
        let Some(end) = closing_at(rest, close) else {
            // Never closed: what follows may be the quoted text itself.
            return out;
        };
        out.push('"');
        rest = &rest[end + close.len_utf8()..];
        previous = Some('"');
    }
    out
}

/// Where in `text` the quote `close` closes: for an apostrophe, the first one
/// not followed by a letter or digit (`'it's here'` closes after `here`).
fn closing_at(text: &str, close: char) -> Option<usize> {
    text.char_indices()
        .find(|&(at, c)| {
            c == close
                && (close != '\''
                    || !text[at + c.len_utf8()..]
                        .chars()
                        .next()
                        .is_some_and(char::is_alphanumeric))
        })
        .map(|(at, _)| at)
}

/// Punctuation that may wrap an address or a link (`<maya@…>`, `(https://…)`).
fn is_wrapper(c: char) -> bool {
    matches!(
        c,
        '<' | '>' | '(' | ')' | '[' | ']' | '{' | '}' | ',' | ';' | ':' | '.' | '!' | '?'
    )
}

/// Append `word` to `out`, with an address, a link or an opaque token in it
/// replaced (see [`job_error`]).
fn mask_word(word: &str, out: &mut String) {
    let start = word.find(|c: char| !is_wrapper(c)).unwrap_or(word.len());
    let end = word[start..]
        .rfind(|c: char| !is_wrapper(c))
        .map_or(start, |at| {
            start + at + word[start + at..].chars().next().map_or(0, char::len_utf8)
        });
    let (lead, core, trail) = (&word[..start], &word[start..end], &word[end..]);
    out.push_str(lead);
    if let Some((scheme, after)) = core.split_once("://")
        && is_scheme(scheme)
    {
        let authority_end = after.find(['/', '?', '#']).unwrap_or(after.len());
        let authority = &after[..authority_end];
        let host = authority.rsplit('@').next().unwrap_or(authority);
        out.push_str(scheme);
        out.push_str("://");
        out.push_str(host);
        if authority_end < after.len() || host.len() < authority.len() {
            out.push_str("/…");
        }
    } else if is_address(core) {
        out.push_str("[address]");
    } else if core.chars().count() >= OPAQUE_RUN
        && core
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '/' | '=' | '_' | '-' | '.'))
    {
        out.push_str("[…]");
    } else {
        out.push_str(core);
    }
    out.push_str(trail);
}

/// A URL scheme: a letter, then letters, digits, `+`, `-` or `.`.
fn is_scheme(scheme: &str) -> bool {
    scheme
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic())
        && scheme
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
}

/// Whether `word` holds an email address: something, `@`, a dotted domain.
fn is_address(word: &str) -> bool {
    word.rsplit_once('@').is_some_and(|(local, domain)| {
        !local.is_empty() && domain.split('.').filter(|label| !label.is_empty()).count() >= 2
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn ids_and_names_pass_data_does_not() {
        for ok in [
            "0192a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a5b",
            "4bf92f3577b34da6a3ce929d0e0e4736",
            "/api/v1/w/{workspace_id}/members",
            "connectors.sync_stream",
            "crates/kernel/src/error.rs:162",
            "campbooks_kernel::error",
        ] {
            assert!(looks_like_an_id(ok), "{ok}");
        }
        for leak in [
            "alice@example.com",
            "Q4 restructuring plan",
            "eyJhbGciOiJSUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4ifQ.abc",
            "ya29.a0AfH6SMBx3examplelongaccesstokenvalue1234567890",
            "",
        ] {
            assert!(!looks_like_an_id(leak), "{leak}");
        }
    }

    #[test]
    fn error_kinds_keep_the_head_of_the_message() {
        assert_eq!(
            error_kind("database error 23505 on constraint members_email_key in table members"),
            "database error 23505 on constraint members_email_key in table members"
        );
        assert_eq!(
            error_kind(
                "provider answered 401 for https://gmail.googleapis.com/gmail/v1/users/me: Bearer x"
            ),
            "provider answered 401 for https://gmail.googleapis.com/…"
        );
        assert_eq!(
            error_kind("job failed: Please find the board pack attached"),
            "job failed"
        );
        assert_eq!(
            error_kind("sync stopped (maya@example.com said no)"),
            "sync stopped"
        );
        assert_eq!(
            error_kind(&"x ".repeat(200)).chars().count(),
            MAX_KIND_LEN + 1
        );
    }

    use super::*;

    const MAIL: &str = "Maya <maya@northwind-studio.example>";

    #[test]
    fn json_errors_keep_no_content() {
        let err = serde_json::from_str::<u32>(&format!("{MAIL:?}")).unwrap_err();
        assert!(err.to_string().contains("maya@"), "serde quotes the value");
        let redacted = json(&err);
        assert!(!redacted.contains("maya"), "{redacted}");
        assert!(redacted.starts_with("Data error at line 1"), "{redacted}");
    }

    #[test]
    fn a_json_error_inside_another_is_found() {
        #[derive(Debug, thiserror::Error)]
        #[error("reading a document: {0}")]
        struct Wrapper(#[source] serde_json::Error);

        let inner = serde_json::from_str::<u32>(&format!("{MAIL:?}")).unwrap_err();
        let wrapped = Wrapper(inner);
        assert!(wrapped.to_string().contains("maya@"));
        let redacted = error(&wrapped);
        assert!(!redacted.contains("maya"), "{redacted}");
    }

    #[test]
    fn other_errors_keep_their_own_message_only() {
        let io = std::io::Error::other("disk full");
        assert_eq!(error(&io), "disk full");
        assert_eq!(sqlx(&sqlx::Error::RowNotFound), "no row found");
    }

    #[test]
    fn job_errors_keep_the_wording_and_hide_what_was_quoted() {
        let reply = format!(
            "provider answered 400: {{\"error\":\"bad_request\",\"echo\":\"Subject: Q4 plan from {MAIL}\"}}\nHi Sam,\nthe Q4 plan is attached."
        );
        let stored = job_error(&reply);
        assert_eq!(stored, r#"provider answered 400: {"…":"…","…":"…"} […]"#);
        for (quote, expected) in [
            ("refused 'Q4 plan' for now", r#"refused "…" for now"#),
            ("refused `Q4 plan`", r#"refused "…""#),
            ("refused “Q4 plan” today", r#"refused "…" today"#),
            ("refused «Q4 plan»", r#"refused "…""#),
            ("refused \"Q4 plan from maya", r#"refused "…"#),
            (
                "the batch's inbox does not exist yet",
                "the batch's inbox does not exist yet",
            ),
            (
                "the users' list is 'it's here' now",
                r#"the users' list is "…" now"#,
            ),
        ] {
            assert_eq!(job_error(quote), expected, "{quote}");
        }
    }

    #[test]
    fn job_errors_hide_addresses_links_and_tokens() {
        // An opaque run as long as a bearer token (built, not a literal).
        let opaque = "Qx7-".repeat(12);
        let message = format!(
            "send to {MAIL} failed (mailto:maya@northwind-studio.example), see \
             https://demo@mail.example.com/v1/messages?q=Q4+plan&token=x, \
             token {opaque} for 01927a00-0000-7000-8000-00000000a001 at\tline 3"
        );
        let stored = job_error(&message);
        assert_eq!(
            stored,
            "send to Maya <[address]> failed ([address]), see https://mail.example.com/…, \
             token […] for 01927a00-0000-7000-8000-00000000a001 at line 3"
        );
        assert_eq!(
            job_error("GET https://api.example.com failed"),
            "GET https://api.example.com failed"
        );
        assert_eq!(
            job_error("decide provider answered 400"),
            "decide provider answered 400"
        );
        assert_eq!(job_error("database error 22P02"), "database error 22P02");
    }
}
