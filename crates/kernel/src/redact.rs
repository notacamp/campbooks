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

#[cfg(test)]
mod tests {
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
}
