//! Module and feature names: what is allowed, and the spellings templates use.

use anyhow::{Result, bail};

/// Longest module or feature name. Module names become Postgres schema names
/// (63 bytes at most) and appear inside longer identifiers.
const MAX_LEN: usize = 32;

/// Names no module may take.
///
/// - Platform crates and directories, the kernel's schema, and the client
///   shell's i18n namespace (`shell`).
/// - SQL reserved words: the module name is a Postgres schema name.
/// - JavaScript reserved words: the client imports the module's messages
///   under its name.
const RESERVED_MODULES: &[&str] = &[
    // Platform
    "ai",
    "api",
    "app",
    "bff",
    "campbooks",
    "clients",
    "crates",
    "kernel",
    "lib",
    "migrations",
    "modules",
    "openapi",
    "public",
    "server",
    "shell",
    "test",
    "testing",
    "web",
    "xtask",
    "information_schema",
    // Postgres reserved key words
    "all",
    "analyse",
    "analyze",
    "and",
    "any",
    "array",
    "as",
    "asc",
    "asymmetric",
    "both",
    "case",
    "cast",
    "check",
    "collate",
    "column",
    "constraint",
    "create",
    "current_catalog",
    "current_date",
    "current_role",
    "current_time",
    "current_timestamp",
    "current_user",
    "default",
    "deferrable",
    "desc",
    "distinct",
    "do",
    "else",
    "end",
    "except",
    "false",
    "fetch",
    "for",
    "foreign",
    "from",
    "grant",
    "group",
    "having",
    "in",
    "initially",
    "intersect",
    "into",
    "lateral",
    "leading",
    "limit",
    "localtime",
    "localtimestamp",
    "not",
    "null",
    "offset",
    "on",
    "only",
    "or",
    "order",
    "placing",
    "primary",
    "references",
    "returning",
    "select",
    "session_user",
    "some",
    "symmetric",
    "system_user",
    "table",
    "then",
    "to",
    "trailing",
    "true",
    "union",
    "unique",
    "user",
    "using",
    "variadic",
    "when",
    "where",
    "window",
    "with",
    // JavaScript reserved words
    "await",
    "break",
    "catch",
    "class",
    "const",
    "continue",
    "debugger",
    "delete",
    "enum",
    "export",
    "extends",
    "finally",
    "function",
    "if",
    "implements",
    "import",
    "instanceof",
    "interface",
    "let",
    "new",
    "package",
    "private",
    "protected",
    "return",
    "static",
    "super",
    "switch",
    "this",
    "throw",
    "try",
    "typeof",
    "var",
    "void",
    "while",
    "yield",
];

/// Names no feature may take: Rust keywords (a feature is a Rust module) and
/// the folders a module crate reserves for itself.
const RESERVED_FEATURES: &[&str] = &[
    // Folders and files of a module crate
    "domain", "events", "features", "scopes", "tests",
    // Rust keywords, strict and reserved
    "abstract", "as", "async", "await", "become", "box", "break", "const", "continue", "crate",
    "do", "dyn", "else", "enum", "extern", "false", "final", "fn", "for", "gen", "if", "impl",
    "in", "let", "loop", "macro", "match", "mod", "move", "mut", "override", "priv", "pub", "ref",
    "return", "self", "static", "struct", "super", "trait", "true", "try", "type", "typeof",
    "unsafe", "unsized", "use", "virtual", "where", "while", "yield",
];

/// What a name is for; each kind has its own reserved words.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A domain module (`crates/modules/<name>`).
    Module,
    /// A use case inside a module (`src/features/<name>`).
    Feature,
}

impl Kind {
    fn label(self) -> &'static str {
        match self {
            Self::Module => "module",
            Self::Feature => "feature",
        }
    }

    fn reserved(self) -> &'static [&'static str] {
        match self {
            Self::Module => RESERVED_MODULES,
            Self::Feature => RESERVED_FEATURES,
        }
    }
}

/// Fails unless `name` is a `snake_case` identifier (`[a-z][a-z0-9]*` words
/// joined by single underscores), at most [`MAX_LEN`] bytes, and not reserved.
pub fn validate(kind: Kind, name: &str) -> Result<()> {
    let label = kind.label();
    let words: Vec<&str> = name.split('_').collect();
    let well_formed = name.len() <= MAX_LEN
        && name.starts_with(|c: char| c.is_ascii_lowercase())
        && words.iter().all(|word| {
            !word.is_empty()
                && word
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        });
    if !well_formed {
        bail!(
            "`{name}` is not a valid {label} name: use snake_case (`[a-z][a-z0-9]*` words joined \
             by single underscores, e.g. `mail_sync`), at most {MAX_LEN} characters"
        );
    }
    if kind.reserved().contains(&name) || (kind == Kind::Module && name.starts_with("pg_")) {
        bail!("`{name}` is reserved and cannot be a {label} name; pick a more specific one");
    }
    Ok(())
}

/// `mail_sync` → `mail-sync` (crate names, URLs, OpenSpec change names).
pub fn kebab(name: &str) -> String {
    name.replace('_', "-")
}

/// `mail_sync` → `MailSync` (Rust and TypeScript type names).
pub fn pascal(name: &str) -> String {
    name.split('_').map(capitalize).collect()
}

/// `mail_sync` → `mailSync` (TypeScript value names).
pub fn camel(name: &str) -> String {
    let pascal = pascal(name);
    let mut chars = pascal.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_ascii_lowercase().to_string() + chars.as_str()
    })
}

/// `mail_sync` → `Mail sync` (headings, summaries).
pub fn title(name: &str) -> String {
    capitalize(&name.replace('_', " "))
}

fn capitalize(word: &str) -> String {
    let mut chars = word.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_ascii_uppercase().to_string() + chars.as_str()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_snake_case() {
        for name in ["mail", "mail_sync", "money2", "a1_b2_c3"] {
            validate(Kind::Module, name).unwrap();
            validate(Kind::Feature, name).unwrap();
        }
    }

    #[test]
    fn rejects_malformed_names() {
        for name in [
            "",
            "Mail",
            "mail-sync",
            "_mail",
            "mail_",
            "mail__sync",
            "2mail",
            "mail sync",
            "mäil",
            "a_very_long_module_name_that_never_ends",
        ] {
            let error = validate(Kind::Module, name).unwrap_err().to_string();
            assert!(error.contains("snake_case"), "{name}: {error}");
        }
    }

    #[test]
    fn rejects_reserved_names() {
        for name in ["kernel", "shell", "user", "order", "delete", "pg_stats"] {
            let error = validate(Kind::Module, name).unwrap_err().to_string();
            assert!(error.contains("reserved"), "{name}: {error}");
        }
        for name in ["domain", "mod", "type", "tests"] {
            assert!(validate(Kind::Feature, name).is_err(), "{name}");
        }
        // Reserved words are per kind.
        validate(Kind::Module, "domain").unwrap();
        validate(Kind::Feature, "user").unwrap();
    }

    #[test]
    fn spellings() {
        assert_eq!(kebab("mail_sync"), "mail-sync");
        assert_eq!(pascal("mail_sync"), "MailSync");
        assert_eq!(pascal("money2_v3"), "Money2V3");
        assert_eq!(camel("mail_sync"), "mailSync");
        assert_eq!(camel("system"), "system");
        assert_eq!(title("mail_sync"), "Mail sync");
    }
}
