//! Architecture rules (see `AGENTS.md` → "Architecture rules").
//!
//! Everything here is a pure function over plain data so the rules are unit
//! tested with fixtures; `main.rs` only gathers the data from `cargo metadata`
//! and the file system.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::LazyLock;

use regex::Regex;
use serde::Deserialize;

/// Where a workspace crate sits in the architecture, derived from its path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kind {
    /// `crates/kernel`
    Kernel,
    /// `crates/ai`
    Ai,
    /// `crates/server`
    Server,
    /// `crates/modules/<name>`
    Module(String),
    /// `xtask`
    Xtask,
    /// `crates/tracking-plan`: repository tooling CI runs without building
    /// the server (the tracking plan, ADR 0020).
    Tooling,
    /// Anywhere else — not allowed until the rules know about it.
    Unknown,
}

impl Kind {
    /// Classify a crate by its directory relative to the workspace root
    /// (`/`-separated).
    pub fn from_path(rel_dir: &str) -> Self {
        let parts: Vec<&str> = rel_dir.trim_matches('/').split('/').collect();
        match parts.as_slice() {
            ["crates", "kernel"] => Self::Kernel,
            ["crates", "ai"] => Self::Ai,
            ["crates", "server"] => Self::Server,
            ["crates", "modules", name] => Self::Module((*name).to_owned()),
            ["xtask"] => Self::Xtask,
            ["crates", "tracking-plan"] => Self::Tooling,
            _ => Self::Unknown,
        }
    }
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Kernel => f.write_str("kernel"),
            Self::Ai => f.write_str("ai"),
            Self::Server => f.write_str("server"),
            Self::Module(name) => write!(f, "module `{name}`"),
            Self::Xtask => f.write_str("xtask"),
            Self::Tooling => f.write_str("tooling"),
            Self::Unknown => f.write_str("unclassified crate"),
        }
    }
}

/// A workspace crate and the workspace crates it depends on (any dep kind).
#[derive(Debug, Clone)]
pub struct Crate {
    /// Package name.
    pub name: String,
    /// Directory relative to the workspace root.
    pub path: String,
    /// Package version.
    pub version: String,
    /// Names of the *workspace* crates it depends on.
    pub deps: Vec<String>,
}

/// One `[[allow]]` entry of `architecture.toml`.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Allow {
    /// Depending module (directory name).
    pub from: String,
    /// Module it may depend on.
    pub to: String,
    /// Why events are not enough.
    pub reason: String,
}

/// `architecture.toml`.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArchitectureFile {
    /// Allowed module → module dependencies.
    #[serde(default)]
    pub allow: Vec<Allow>,
    /// The access rules' reviewed exceptions (ADR 0017).
    #[serde(default)]
    pub access: crate::access::AccessFile,
}

/// Names of the modules (directory names under `crates/modules`).
pub fn module_names(crates: &[Crate]) -> BTreeSet<String> {
    crates
        .iter()
        .filter_map(|c| match Kind::from_path(&c.path) {
            Kind::Module(name) => Some(name),
            _ => None,
        })
        .collect()
}

/// Dependency-direction rules between workspace crates.
pub fn check_dependencies(crates: &[Crate], allow: &[Allow]) -> Vec<String> {
    let kinds: BTreeMap<&str, Kind> = crates
        .iter()
        .map(|c| (c.name.as_str(), Kind::from_path(&c.path)))
        .collect();
    let mut violations = Vec::new();

    for krate in crates {
        let from = Kind::from_path(&krate.path);
        if from == Kind::Unknown {
            violations.push(format!(
                "`{}` lives at `{}`, which is not a known crate location \
                 (crates/kernel, crates/ai, crates/server, crates/modules/<name>, xtask, \
                 crates/tracking-plan)",
                krate.name, krate.path
            ));
            continue;
        }
        for dep in &krate.deps {
            let Some(to) = kinds.get(dep.as_str()) else {
                continue;
            };
            if let Some(why) = edge_violation(&from, to, allow) {
                violations.push(format!(
                    "`{}` ({from}) must not depend on `{dep}` ({to}): {why}",
                    krate.name
                ));
            }
        }
    }
    violations
}

#[expect(
    clippy::match_same_arms,
    reason = "one arm per rule reads as a table; merging arms would hide the order they are checked in"
)]
fn edge_violation(from: &Kind, to: &Kind, allow: &[Allow]) -> Option<&'static str> {
    if *to == Kind::Xtask {
        return Some("nothing depends on xtask");
    }
    if *to == Kind::Server && *from != Kind::Xtask {
        return Some("only xtask may depend on the server");
    }
    if *to == Kind::Tooling && *from != Kind::Xtask {
        return Some("only xtask may depend on the tooling crates");
    }
    match (from, to) {
        (Kind::Kernel, _) => Some("the kernel depends on no workspace crate"),
        (Kind::Ai, Kind::Kernel) => None,
        (Kind::Ai, _) => Some("campbooks-ai depends only on the kernel"),
        (Kind::Module(_), Kind::Kernel | Kind::Ai) => None,
        (Kind::Module(a), Kind::Module(b)) => {
            if allow.iter().any(|e| &e.from == a && &e.to == b) {
                None
            } else {
                Some(
                    "modules don't import modules; publish a domain event, or add an \
                     [[allow]] entry to architecture.toml (reviewed like an ADR)",
                )
            }
        }
        (Kind::Module(_), _) => Some("modules may depend only on the kernel and campbooks-ai"),
        (Kind::Server | Kind::Xtask, _) => None,
        (Kind::Tooling, _) => Some(
            "the tooling crates depend on no workspace crate, so CI runs them without \
             building the server",
        ),
        (Kind::Unknown, _) => Some("unclassified crate"),
    }
}

/// Every allowlist entry names two existing, distinct modules and a reason.
pub fn check_allowlist(allow: &[Allow], modules: &BTreeSet<String>) -> Vec<String> {
    let mut violations = Vec::new();
    for entry in allow {
        for name in [&entry.from, &entry.to] {
            if !modules.contains(name) {
                violations.push(format!(
                    "architecture.toml: [[allow]] {} -> {} names unknown module `{name}`",
                    entry.from, entry.to
                ));
            }
        }
        if entry.from == entry.to {
            violations.push(format!(
                "architecture.toml: [[allow]] {0} -> {0} is a self-reference",
                entry.from
            ));
        }
        if entry.reason.trim().is_empty() {
            violations.push(format!(
                "architecture.toml: [[allow]] {} -> {} needs a non-empty reason",
                entry.from, entry.to
            ));
        }
    }
    violations
}

/// The Postgres schema of the kernel's own infrastructure (event log, job
/// queue). Its migrations are `<ts>_kernel__<desc>.sql`; no module may be
/// named after it.
pub const KERNEL_SCHEMA: &str = "kernel";

/// The Postgres schema of the server's web BFF (sessions). Its migrations are
/// `<ts>_bff__<desc>.sql`; no module may be named after it.
pub const BFF_SCHEMA: &str = "bff";

/// Schemas owned by infrastructure rather than a module.
pub const INFRASTRUCTURE_SCHEMAS: [&str; 2] = [KERNEL_SCHEMA, BFF_SCHEMA];

fn is_infrastructure(schema: &str) -> bool {
    INFRASTRUCTURE_SCHEMAS.contains(&schema)
}

static MIGRATION_NAME: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(\d{14})_([a-z0-9_]+?)__([a-z0-9_]+)\.sql$")
        .unwrap_or_else(|e| unreachable!("static regex compiles: {e}"))
});

/// No module may take the name of a non-module operation owner (the change
/// feed's `changes`): operation ids are prefixed by their owner.
pub fn check_reserved_owner(modules: &BTreeSet<String>, owner: &str) -> Vec<String> {
    if modules.contains(owner) {
        vec![format!(
            "crates/modules/{owner}: `{owner}` is reserved for operations the server owns"
        )]
    } else {
        Vec::new()
    }
}

/// Migration files are `<14-digit timestamp>_<owner>__<description>.sql`,
/// where the owner is an existing module or an infrastructure schema
/// (`kernel`, `bff`), and have unique timestamps. No module may take an
/// infrastructure schema's name.
pub fn check_migrations(file_names: &[String], modules: &BTreeSet<String>) -> Vec<String> {
    let mut violations = Vec::new();
    let mut versions: BTreeMap<&str, &str> = BTreeMap::new();
    for reserved in INFRASTRUCTURE_SCHEMAS {
        if modules.contains(reserved) {
            violations.push(format!(
                "crates/modules/{reserved}: `{reserved}` is reserved for an infrastructure schema"
            ));
        }
    }

    for name in file_names {
        let Some(caps) = MIGRATION_NAME.captures(name) else {
            violations.push(format!(
                "migrations/{name}: expected `<YYYYMMDDHHMMSS>_<module>__<description>.sql` \
                 (lower-case, digits and underscores)"
            ));
            continue;
        };
        let (version, module) = (
            caps.get(1).map_or("", |m| m.as_str()),
            caps.get(2).map_or("", |m| m.as_str()),
        );
        if !is_infrastructure(module) && !modules.contains(module) {
            let known: Vec<_> = modules.iter().map(String::as_str).collect();
            violations.push(format!(
                "migrations/{name}: `{module}` is not a module, `{KERNEL_SCHEMA}` or \
                 `{BFF_SCHEMA}` (known modules: {})",
                known.join(", ")
            ));
        }
        if let Some(other) = versions.insert(version, name) {
            violations.push(format!(
                "migrations/{name}: timestamp {version} is already used by {other}"
            ));
        }
    }
    violations
}

static SQL_NOISE: LazyLock<Regex> = LazyLock::new(|| {
    // Comments, string literals and dollar-quoted bodies' delimiters are not
    // schema references.
    Regex::new(r"(?s)--[^\n]*|/\*.*?\*/|'(?:[^']|'')*'")
        .unwrap_or_else(|e| unreachable!("static regex compiles: {e}"))
});

static SCHEMA_STATEMENT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"\b(?:create|alter|drop)\s+schema\s+(?:if\s+(?:not\s+)?exists\s+)?([a-z_][a-z0-9_]*)",
    )
    .unwrap_or_else(|e| unreachable!("static regex compiles: {e}"))
});

/// A function's own `SET search_path = pg_catalog, pg_temp`: the hygiene a
/// `SECURITY DEFINER` function needs, so names in its body cannot be
/// shadowed by the caller's objects. The only mention of `search_path` a
/// migration may make. Exactly those two entries, `pg_temp` last: a match
/// followed by `,` (another entry), by `;` or by the end of the file (a
/// statement changing the session's path) captures group 2 and is refused
/// (campbooks-cloud#213). `pg_temp` must end the name: a following
/// identifier character, `$` included (`pg_temp$x` is another schema),
/// captures group 1 and is refused too (campbooks-cloud#214; the `regex`
/// crate has no look-ahead for `pg_temp(?![\w$])`).
static PINNED_SEARCH_PATH: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\bset\s+search_path\s*(?:=|to)\s*pg_catalog\s*,\s*pg_temp([\w$]*)(\s*(?:[,;]|$))?")
        .unwrap_or_else(|e| unreachable!("static regex compiles: {e}"))
});

/// A string literal naming `search_path` (`set_config('search_path', …)`)
/// changes the path at run time: refused, as string literals are otherwise
/// dropped before the schema checks read a migration (campbooks-cloud#214).
fn names_search_path_in_a_string(sql: &str) -> bool {
    SQL_NOISE.find_iter(sql).any(|m| {
        let text = m.as_str();
        text.starts_with('\'') && text.to_lowercase().contains("search_path")
    })
}

static QUALIFIED_NAME: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b([a-z_][a-z0-9_]*)\s*\.\s*[a-z_]")
        .unwrap_or_else(|e| unreachable!("static regex compiles: {e}"))
});

/// Each migration touches only its owner's schema: it creates, alters or
/// drops no other schema, never qualifies a name with another module's (or
/// the kernel's) schema, and never changes `search_path` (a function's own
/// pinned `SET search_path = pg_catalog, pg_temp` aside). `files` are
/// `(file name, SQL)`; files with a malformed name are reported by
/// [`check_migrations`] and skipped here.
pub fn check_migration_schemas(
    files: &[(String, String)],
    modules: &BTreeSet<String>,
) -> Vec<String> {
    let mut violations = Vec::new();
    for (name, raw) in files {
        let Some(owner) = MIGRATION_NAME
            .captures(name)
            .and_then(|caps| caps.get(2))
            .map(|m| m.as_str())
        else {
            continue;
        };
        let sql = SQL_NOISE
            .replace_all(raw, " ")
            .to_lowercase()
            .replace('"', "");
        let mut touched = BTreeSet::new();
        for caps in SCHEMA_STATEMENT.captures_iter(&sql) {
            let schema = caps.get(1).map_or("", |m| m.as_str());
            if schema != owner {
                touched.insert(schema.to_owned());
            }
        }
        for caps in QUALIFIED_NAME.captures_iter(&sql) {
            let schema = caps.get(1).map_or("", |m| m.as_str());
            let known = is_infrastructure(schema) || modules.contains(schema);
            if known && schema != owner {
                touched.insert(schema.to_owned());
            }
        }
        for schema in touched {
            violations.push(format!(
                "migrations/{name}: touches schema `{schema}`; a migration of `{owner}` may \
                 touch only the `{owner}` schema (reference other modules by ID)"
            ));
        }
        let pinned = PINNED_SEARCH_PATH
            .captures_iter(&sql)
            .filter(|caps| caps.get(1).is_some_and(|tail| tail.is_empty()) && caps.get(2).is_none())
            .count();
        if sql.matches("search_path").count() > pinned || names_search_path_in_a_string(raw) {
            violations.push(format!(
                "migrations/{name}: must not change `search_path`; qualify names with `{owner}.`"
            ));
        }
    }
    violations
}

/// Background registrations (job kinds, event handlers, schedules) are
/// named `<module>.<name>` of the declaring module and unique. `errors` is
/// what building the registry reported.
pub fn check_background(errors: &[String]) -> Vec<String> {
    errors
        .iter()
        .map(|e| format!("background registration: {e}"))
        .collect()
}

/// Every workspace crate carries the version in the `VERSION` file.
pub fn check_versions(version_file: &str, crates: &[Crate]) -> Vec<String> {
    let expected = version_file.trim();
    crates
        .iter()
        .filter(|c| c.version != expected)
        .map(|c| {
            format!(
                "`{}` is version {} but VERSION says {expected}; keep [workspace.package] \
                 version in Cargo.toml equal to VERSION",
                c.name, c.version
            )
        })
        .collect()
}

/// HTTP methods an OpenAPI path item may hold operations under.
pub const OPERATION_METHODS: [&str; 8] = [
    "get", "put", "post", "delete", "options", "head", "patch", "trace",
];

/// The security schemes an operation may name.
#[derive(Debug, Clone, Copy)]
pub struct Schemes<'a> {
    /// The OAuth bearer scheme; requirements list scopes of the catalogue.
    pub bearer: &'a str,
    /// The web BFF's session-cookie scheme; scope-less, and only on `/bff/`
    /// paths.
    pub session: &'a str,
}

/// Path prefix of the web BFF's operations, the only ones that may use the
/// session-cookie scheme.
const BFF_PATH_PREFIX: &str = "/bff/";

/// Every operation of the generated OpenAPI document states its security
/// explicitly: public (`security(())`, i.e. `[{}]`), the `bearer` scheme
/// with exactly the scope from the catalogue that its `Scoped<S>` extractor
/// requires, or — for the web BFF's `/bff/*` operations only — the `session`
/// cookie scheme, which has no scopes. Nothing is left to a document-wide
/// default.
pub fn check_operation_security(
    spec: &serde_json::Value,
    schemes: Schemes<'_>,
    catalogue: &BTreeSet<String>,
) -> Vec<String> {
    let mut violations = Vec::new();
    let Some(paths) = spec.get("paths").and_then(serde_json::Value::as_object) else {
        return violations;
    };
    for (path, item) in paths {
        for method in OPERATION_METHODS {
            let Some(operation) = item.get(method) else {
                continue;
            };
            let name = format!("{} {path}", method.to_uppercase());
            let bearer = schemes.bearer;
            match operation
                .get("security")
                .and_then(serde_json::Value::as_array)
            {
                None => violations.push(format!(
                    "OpenAPI operation `{name}` declares no `security`: require a scope with \
                     `security((\"{bearer}\" = [\"<module>:<action>\"]))` and the \
                     `Scoped<S>` extractor, or mark it public with `security(())`"
                )),
                Some(requirements) => {
                    let on_bff = path.starts_with(BFF_PATH_PREFIX);
                    violations.extend(check_requirements(
                        &name,
                        requirements,
                        schemes,
                        on_bff,
                        catalogue,
                    ));
                }
            }
        }
    }
    violations
}

fn check_requirements(
    operation: &str,
    requirements: &[serde_json::Value],
    schemes: Schemes<'_>,
    on_bff: bool,
    catalogue: &BTreeSet<String>,
) -> Vec<String> {
    let scheme = schemes.bearer;
    let mut violations = Vec::new();
    for requirement in requirements {
        let Some(requirement) = requirement.as_object() else {
            violations.push(format!(
                "OpenAPI operation `{operation}`: malformed security requirement"
            ));
            continue;
        };
        for (name, scopes) in requirement {
            let scopes: Vec<&str> = scopes
                .as_array()
                .map(|list| list.iter().filter_map(serde_json::Value::as_str).collect())
                .unwrap_or_default();
            if name == schemes.session {
                if !on_bff {
                    violations.push(format!(
                        "OpenAPI operation `{operation}` uses the `{name}` cookie scheme, which \
                         only the web BFF's `{BFF_PATH_PREFIX}*` operations may use; the API \
                         takes bearer tokens"
                    ));
                }
                if !scopes.is_empty() {
                    violations.push(format!(
                        "OpenAPI operation `{operation}`: the `{name}` scheme takes no scopes"
                    ));
                }
                continue;
            }
            if name != scheme {
                violations.push(format!(
                    "OpenAPI operation `{operation}` uses security scheme `{name}`; only \
                     `{scheme}` and `{}` exist",
                    schemes.session
                ));
                continue;
            }
            if scopes.is_empty() {
                violations.push(format!(
                    "OpenAPI operation `{operation}` requires a `{scheme}` token but no scope; \
                     every protected endpoint requires one"
                ));
            }
            for scope in scopes {
                if !catalogue.contains(scope) {
                    violations.push(format!(
                        "OpenAPI operation `{operation}` requires scope `{scope}`, which no \
                         module declares (see `campbooks-server scopes`)"
                    ));
                }
            }
        }
    }
    violations
}

/// An operation's place in the catalogue: the module whose router serves it.
pub type Owners = BTreeMap<(String, String), String>;

static OPERATION_ID: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^([a-z][a-z0-9_]*)\.([a-z][a-z0-9_]*)$")
        .unwrap_or_else(|e| unreachable!("static regex compiles: {e}"))
});

/// The `x-campbooks` keys every operation declares (ADR 0013); `undo` is
/// optional.
const REQUIRED_META: [&str; 5] = ["mutates", "automation", "assistant", "risk", "records"];

/// Every operation of the generated document is a catalogue entry
/// (ADR 0013): an `operationId` `<module>.<verb>[_<noun>]` naming the module
/// that serves it, unique; a complete `x-campbooks` block whose `mutates`
/// matches the method (POST/PUT/PATCH/DELETE mutate, GET/HEAD don't), whose
/// records are the module's own `<module>.<record>` types, and whose `undo`
/// names an existing mutating operation. `owners` maps `(METHOD, path)` to
/// the module whose router declares it.
pub fn check_operations(spec: &serde_json::Value, owners: &Owners) -> Vec<String> {
    let mut violations = Vec::new();
    let Some(paths) = spec.get("paths").and_then(serde_json::Value::as_object) else {
        return violations;
    };
    // id → (operation, mutates)
    let mut ids: BTreeMap<String, (String, Option<bool>)> = BTreeMap::new();
    let mut undos: Vec<(String, String)> = Vec::new();
    for (path, item) in paths {
        for method in OPERATION_METHODS {
            let Some(operation) = item.get(method) else {
                continue;
            };
            let method = method.to_uppercase();
            let name = format!("{method} {path}");
            let owner = owners.get(&(method.clone(), path.clone()));
            if owner.is_none() {
                violations.push(format!(
                    "OpenAPI operation `{name}` is not served by any module's router"
                ));
            }
            let Some(id) = operation
                .get("operationId")
                .and_then(serde_json::Value::as_str)
            else {
                violations.push(format!(
                    "OpenAPI operation `{name}` has no `operationId`: add \
                     `operation_id = \"<module>.<verb>[_<noun>]\"` to its `#[utoipa::path]`"
                ));
                continue;
            };
            match (OPERATION_ID.captures(id), owner) {
                (None, _) => violations.push(format!(
                    "OpenAPI operation `{name}`: operationId `{id}` must be \
                     `<module>.<verb>[_<noun>]`, lower-case snake case"
                )),
                (Some(caps), Some(owner))
                    if caps.get(1).map(|m| m.as_str()) != Some(owner.as_str()) =>
                {
                    violations.push(format!(
                        "OpenAPI operation `{name}`: operationId `{id}` must start with \
                         `{owner}.`, the module that serves it"
                    ));
                }
                _ => {}
            }
            let meta = check_meta(&name, &method, operation, owner, &mut violations);
            if let Some(undo) = meta.as_ref().and_then(|m| m.undo.clone()) {
                undos.push((id.to_owned(), undo));
            }
            if let Some((other, _)) =
                ids.insert(id.to_owned(), (name.clone(), meta.map(|m| m.mutates)))
            {
                violations.push(format!(
                    "operationId `{id}` is used by both `{other}` and `{name}`; ids are unique"
                ));
            }
        }
    }
    for (id, undo) in undos {
        let mutates = ids.get(&id).and_then(|(_, mutates)| *mutates);
        if mutates == Some(false) {
            violations.push(format!(
                "operation `{id}` names an undo (`{undo}`) but does not mutate; only \
                 mutating operations can be undone"
            ));
        }
        match ids.get(&undo) {
            None => violations.push(format!(
                "operation `{id}`: undo target `{undo}` is not an operation"
            )),
            Some((_, Some(false))) => violations.push(format!(
                "operation `{id}`: undo target `{undo}` does not mutate"
            )),
            // An operation may be its own undo when sending the previous values
            // back to it reverses it (`schedule.update_event`).
            Some(_) => {}
        }
    }
    violations
}

/// The words of a field name, lower-cased: split at `_`, `-`, `.`, a
/// space, camelCase boundaries, the end of a run of capitals (`IDToken` is
/// `id`, `token`) and before a trailing number (`token2` is `token`, `2`).
pub(crate) fn name_words(name: &str) -> Vec<String> {
    let chars: Vec<char> = name.chars().collect();
    let mut words: Vec<String> = Vec::new();
    let mut word = String::new();
    for (i, &c) in chars.iter().enumerate() {
        if matches!(c, '_' | '-' | '.' | ' ') {
            if !word.is_empty() {
                words.push(std::mem::take(&mut word));
            }
            continue;
        }
        let previous = i.checked_sub(1).map(|j| chars[j]);
        let next = chars.get(i + 1).copied();
        let boundary = previous.is_some_and(|p| {
            let upper = c.is_ascii_uppercase();
            (upper && (p.is_ascii_lowercase() || p.is_ascii_digit()))
                || (upper && p.is_ascii_uppercase() && next.is_some_and(|n| n.is_ascii_lowercase()))
                || (c.is_ascii_digit() && p.is_ascii_alphabetic())
        });
        if boundary && !word.is_empty() {
            words.push(std::mem::take(&mut word));
        }
        word.push(c.to_ascii_lowercase());
    }
    if !word.is_empty() {
        words.push(word);
    }
    words
}

/// Whether a field name in an answer looks like it may carry a secret
/// (campbooks-cloud#260), by its words, in any case, singular or plural: a
/// secret, token, ticket, password, passphrase, credential or code
/// (one-time, invitation, signup), an API key or one-time password, an
/// API, private, access, signing or secret key anywhere, or a URL, URI,
/// link or key last (a link may carry a ticket). A word only starting like
/// one (`tokenizer`, `secretary`, `codec`, `keyboard`) is not.
fn guarded_field(name: &str) -> bool {
    const SECRET: [&str; 10] = [
        "secret",
        "token",
        "ticket",
        "password",
        "passwd",
        "passphrase",
        "credential",
        "code",
        "apikey",
        "otp",
    ];
    const LAST: [&str; 4] = ["url", "uri", "link", "key"];
    const KEYED: [&str; 5] = ["api", "private", "access", "signing", "secret"];
    let words = name_words(name);
    let singular = |word: &str| word.strip_suffix('s').unwrap_or(word).to_owned();
    words.iter().any(|w| SECRET.contains(&singular(w).as_str()))
        || words
            .last()
            .is_some_and(|w| LAST.contains(&singular(w).as_str()))
        || words
            .windows(2)
            .any(|pair| KEYED.contains(&pair[0].as_str()) && singular(&pair[1]) == "key")
}

/// The pseudo-field a redirect answers with: its `Location` (where the
/// browser goes next, which may carry a ticket).
const LOCATION_FIELD: &str = "Location";

/// Why `item_keys` of an event is no secret.
const ITEM_KEYS: &str = "the keys of the read items that made or changed an event \
     (`<message id>:<layer>:item:<n>`, built by the code from ids and a position), which the \
     reader's strip matches to find its event; they open nothing";

/// Secret-looking fields in the answers of mutating operations that show no
/// secret, each with the reason: `(operationId, field, reason)`. Every other
/// such field needs its operation to declare `x-campbooks.secret_once`.
pub const ANSWER_FIELDS_ALLOWED: &[(&str, &str, &str)] = &[
    ("schedule.create_event", "item_keys", ITEM_KEYS),
    ("schedule.update_event", "item_keys", ITEM_KEYS),
    ("schedule.push_event", "item_keys", ITEM_KEYS),
    ("schedule.unpush_event", "item_keys", ITEM_KEYS),
    ("schedule.respond_event", "item_keys", ITEM_KEYS),
    ("schedule.restore_event", "item_keys", ITEM_KEYS),
    (
        "schedule.delete_event",
        "undo_token",
        "a handle that only brings the member's own deleted event back, to the member who \
         deleted it (`schedule.restore_event` checks ownership too); it grants no access",
    ),
    (
        "bff.logout",
        "logout_url",
        "the identity provider's end-session URL, which carries no credential",
    ),
    (
        "identity.register",
        "token_endpoint_auth_method",
        "names how the app signs in to the token endpoint (`none`); no secret",
    ),
    (
        "identity.end_session_post",
        LOCATION_FIELD,
        "the app's registered post-logout redirect URI with the `state` it sent; no credential",
    ),
    (
        "identity.start_passkey_registration",
        "excludeCredentials",
        PASSKEY_IDS,
    ),
    (
        "identity.start_passkey_registration",
        "requireResidentKey",
        RESIDENT_KEY,
    ),
    (
        "identity.start_passkey_registration",
        "residentKey",
        RESIDENT_KEY,
    ),
    (
        "identity.start_passkey_sign_up",
        "excludeCredentials",
        PASSKEY_IDS,
    ),
    (
        "identity.start_passkey_sign_up",
        "requireResidentKey",
        RESIDENT_KEY,
    ),
    (
        "identity.start_passkey_sign_up",
        "residentKey",
        RESIDENT_KEY,
    ),
    (
        "identity.start_passkey_sign_in",
        "allowCredentials",
        PASSKEY_IDS,
    ),
    (
        "identity.register",
        "redirect_uris",
        "the redirect URIs the registering app sent, echoed back; no credential",
    ),
    ("conversations.create_draft", "content_token", CONTENT_TOKEN),
    ("conversations.update_draft", "content_token", CONTENT_TOKEN),
    (
        "conversations.upload_draft_attachment",
        "content_token",
        CONTENT_TOKEN,
    ),
    ("conversations.cancel_send", "content_token", CONTENT_TOKEN),
    ("conversations.send_draft", "content_token", CONTENT_TOKEN),
    ("conversations.create_rule", "content_token", CONTENT_TOKEN),
    ("conversations.update_rule", "content_token", CONTENT_TOKEN),
    ("conversations.remove_rule", "content_token", CONTENT_TOKEN),
    ("conversations.confirm_rule", "content_token", CONTENT_TOKEN),
    ("conversations.decline_rule", "content_token", CONTENT_TOKEN),
    ("conversations.restore_rule", "content_token", CONTENT_TOKEN),
];

const PASSKEY_IDS: &str = "WebAuthn: the ids of passkeys (public, as every authenticator \
     shows them), so the browser offers or skips them; no secret";
const RESIDENT_KEY: &str =
    "WebAuthn: whether the authenticator keeps the passkey (`required`, a flag); no key";

const CONTENT_TOKEN: &str =
    "a version tag of the content on screen, for conflict checks; not a credential";

/// The idempotency layer stores the answers of mutating operations (only
/// theirs: a read is never stored, so it is not checked), unless they
/// declare `x-campbooks.secret_once` (campbooks-cloud#213, item 4). Every
/// field of a schema a mutating operation answers below 400 with is read
/// (through `$ref`, `allOf`, `oneOf`, `anyOf`, `items` and
/// `additionalProperties`), and every redirect (3xx) counts as answering a
/// [`LOCATION_FIELD`] (campbooks-cloud#260): a secret-looking name
/// ([`guarded_field`]) or a redirect needs `secret_once` or an entry in
/// `allowed` with its reason. An entry naming a field no longer answered is
/// stale and reported too.
pub fn check_answer_field_names(
    spec: &serde_json::Value,
    allowed: &[(&str, &str, &str)],
) -> Vec<String> {
    let mut violations = Vec::new();
    let mut used = BTreeSet::new();
    let schemas = spec.pointer("/components/schemas");
    let Some(paths) = spec.get("paths").and_then(serde_json::Value::as_object) else {
        return violations;
    };
    for item in paths.values() {
        for method in OPERATION_METHODS {
            let Some(operation) = item.get(method) else {
                continue;
            };
            let meta = operation.get("x-campbooks");
            let flag = |key| {
                meta.and_then(|m| m.get(key))
                    .and_then(serde_json::Value::as_bool)
                    == Some(true)
            };
            if !flag("mutates") || flag("secret_once") {
                continue;
            }
            let id = operation
                .get("operationId")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default();
            let mut fields = BTreeSet::new();
            let responses = operation
                .get("responses")
                .and_then(serde_json::Value::as_object);
            for (status, response) in responses.into_iter().flatten() {
                if !status.starts_with(['1', '2', '3']) {
                    continue;
                }
                if status.starts_with('3') {
                    fields.insert(LOCATION_FIELD.to_owned());
                }
                let contents = response
                    .get("content")
                    .and_then(serde_json::Value::as_object);
                for media in contents.into_iter().flatten().map(|(_, media)| media) {
                    if let Some(schema) = media.get("schema") {
                        field_names(schema, schemas, &mut BTreeSet::new(), &mut fields);
                    }
                }
            }
            let guarded = |f: &&String| f.as_str() == LOCATION_FIELD || guarded_field(f);
            for field in fields.iter().filter(guarded) {
                if allowed.iter().any(|(op, f, _)| *op == id && f == field) {
                    used.insert((id.to_owned(), field.clone()));
                } else {
                    violations.push(format!(
                        "operation `{id}` answers `{field}`, which looks like a secret: \
                         declare `x-campbooks.secret_once` (the idempotency layer then keeps \
                         no copy), or list it in `ANSWER_FIELDS_ALLOWED` with the reason it \
                         is none"
                    ));
                }
            }
        }
    }
    for (op, field, _) in allowed {
        if !used.contains(&((*op).to_owned(), (*field).to_owned())) {
            violations.push(format!(
                "`ANSWER_FIELDS_ALLOWED` lists `{field}` of `{op}`, which no mutating \
                 operation without `secret_once` answers any more: remove the entry"
            ));
        }
    }
    violations
}

/// The property names `schema` holds, at any depth.
fn field_names(
    schema: &serde_json::Value,
    schemas: Option<&serde_json::Value>,
    seen: &mut BTreeSet<String>,
    names: &mut BTreeSet<String>,
) {
    if let Some(reference) = schema.get("$ref").and_then(serde_json::Value::as_str) {
        let name = reference.rsplit('/').next().unwrap_or_default();
        if seen.insert(name.to_owned())
            && let Some(target) = schemas.and_then(|s| s.get(name))
        {
            field_names(target, schemas, seen, names);
        }
        return;
    }
    for key in ["allOf", "oneOf", "anyOf"] {
        for part in schema
            .get(key)
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
        {
            field_names(part, schemas, seen, names);
        }
    }
    if let Some(properties) = schema
        .get("properties")
        .and_then(serde_json::Value::as_object)
    {
        for (name, property) in properties {
            names.insert(name.clone());
            field_names(property, schemas, seen, names);
        }
    }
    for key in ["items", "additionalProperties"] {
        if let Some(inner) = schema.get(key).filter(|inner| inner.is_object()) {
            field_names(inner, schemas, seen, names);
        }
    }
}

/// The parts of `x-campbooks` the cross-operation checks need.
struct Meta {
    mutates: bool,
    undo: Option<String>,
}

fn check_meta(
    name: &str,
    method: &str,
    operation: &serde_json::Value,
    owner: Option<&String>,
    violations: &mut Vec<String>,
) -> Option<Meta> {
    let Some(meta) = operation.get("x-campbooks") else {
        violations.push(format!(
            "OpenAPI operation `{name}` has no `x-campbooks` extension: add \
             `extensions((\"x-campbooks\" = json!(Operation::query() | Operation::command(..))))`"
        ));
        return None;
    };
    let Some(meta) = meta.as_object() else {
        violations.push(format!(
            "OpenAPI operation `{name}`: `x-campbooks` must be an object"
        ));
        return None;
    };
    for key in REQUIRED_META {
        if !meta.contains_key(key) {
            violations.push(format!(
                "OpenAPI operation `{name}`: `x-campbooks.{key}` is missing"
            ));
        }
    }
    for key in meta.keys() {
        if !REQUIRED_META.contains(&key.as_str())
            && key != "undo"
            && key != "query_in"
            && key != "access"
            && key != "rate_limit"
            && key != "secret_once"
        {
            violations.push(format!(
                "OpenAPI operation `{name}`: unknown key `x-campbooks.{key}`"
            ));
        }
    }
    for key in ["mutates", "automation", "assistant", "secret_once"] {
        if meta.get(key).is_some_and(|v| !v.is_boolean()) {
            violations.push(format!(
                "OpenAPI operation `{name}`: `x-campbooks.{key}` must be a boolean"
            ));
        }
    }
    match meta.get("records") {
        None => {}
        Some(serde_json::Value::Array(records)) => {
            for record in records {
                let record = record.as_str().unwrap_or_default();
                let module = OPERATION_ID
                    .captures(record)
                    .and_then(|caps| caps.get(1))
                    .map(|m| m.as_str());
                match (module, owner) {
                    (None, _) => violations.push(format!(
                        "OpenAPI operation `{name}`: record type `{record}` must be \
                         `<module>.<record>`"
                    )),
                    (Some(module), Some(owner)) if module != owner => {
                        violations.push(format!(
                            "OpenAPI operation `{name}`: record type `{record}` belongs to \
                             `{module}`; an operation touches only its own module's records"
                        ));
                    }
                    _ => {}
                }
            }
        }
        Some(_) => violations.push(format!(
            "OpenAPI operation `{name}`: `x-campbooks.records` must be a list of \
             `<module>.<record>` names"
        )),
    }
    let undo = match meta.get("undo") {
        None => None,
        Some(serde_json::Value::String(undo)) => Some(undo.clone()),
        Some(_) => {
            violations.push(format!(
                "OpenAPI operation `{name}`: `x-campbooks.undo` must be an operationId"
            ));
            None
        }
    };
    check_method(name, method, meta, violations);
    check_rate_limit(name, meta, violations);
    check_risk(name, meta, violations);
    if meta.get("secret_once").is_some()
        && meta.get("mutates").and_then(serde_json::Value::as_bool) != Some(true)
    {
        violations.push(format!(
            "OpenAPI operation `{name}`: `x-campbooks.secret_once` is only for operations that \
             mutate (the idempotency layer stores answers of mutating operations only)"
        ));
    }
    meta.get("mutates")
        .and_then(serde_json::Value::as_bool)
        .map(|mutates| Meta { mutates, undo })
}

/// `mutates` must match the method: writes are POST/PUT/PATCH/DELETE, reads
/// GET/HEAD, or a POST marked `query_in: body` (inputs kept out of URLs).
fn check_method(
    name: &str,
    method: &str,
    meta: &serde_json::Map<String, serde_json::Value>,
    violations: &mut Vec<String>,
) {
    if let Some(mutates) = meta.get("mutates").and_then(serde_json::Value::as_bool) {
        let writes = matches!(method, "POST" | "PUT" | "PATCH" | "DELETE");
        let reads = matches!(method, "GET" | "HEAD");
        if mutates && !writes {
            violations.push(format!(
                "OpenAPI operation `{name}` mutates, so it must be POST, PUT, PATCH or DELETE"
            ));
        }
        let body_query = meta.get("query_in").and_then(serde_json::Value::as_str) == Some("body");
        if body_query && (mutates || method != "POST") {
            violations.push(format!(
                "OpenAPI operation `{name}`: `x-campbooks.query_in` is for reads served by POST \
                 (inputs that must stay out of URLs)"
            ));
        }
        if !mutates && !reads && !body_query {
            violations.push(format!(
                "OpenAPI operation `{name}` does not mutate, so it must be GET or HEAD (or a POST \
                 marked `Operation::body_query()` when its inputs must stay out of URLs)"
            ));
        }
    }
}

/// `risk` is `low` or `high`, and a high-risk operation (it sends mail in
/// the person's name, deletes, or changes who may do what) is never
/// automation's nor the assistant's: a person runs it with their own
/// request (campbooks-cloud#242). Whether a call was "confirmed" is said by
/// the code making it, so it proves no click.
fn check_risk(
    name: &str,
    meta: &serde_json::Map<String, serde_json::Value>,
    violations: &mut Vec<String>,
) {
    if meta
        .get("risk")
        .is_some_and(|risk| !matches!(risk.as_str(), Some("low" | "high")))
    {
        violations.push(format!(
            "OpenAPI operation `{name}`: `x-campbooks.risk` must be \"low\" or \"high\""
        ));
    }
    if meta.get("risk").and_then(serde_json::Value::as_str) != Some("high") {
        return;
    }
    for caller in ["automation", "assistant"] {
        if meta.get(caller).and_then(serde_json::Value::as_bool) == Some(true) {
            violations.push(format!(
                "OpenAPI operation `{name}` is high-risk and opts `x-campbooks.{caller}` in: \
                 a high-risk operation is the person's own request only"
            ));
        }
    }
}

/// `rate_limit` may only make a limit tighter: it is `send`, on a mutating
/// operation (a read declaring a write's or send's class would be
/// meaningless, and no class looser than its method's exists).
fn check_rate_limit(
    name: &str,
    meta: &serde_json::Map<String, serde_json::Value>,
    violations: &mut Vec<String>,
) {
    let Some(class) = meta.get("rate_limit") else {
        return;
    };
    if class.as_str() != Some("send") {
        violations.push(format!(
            "OpenAPI operation `{name}`: `x-campbooks.rate_limit` must be \"send\""
        ));
    }
    if meta.get("mutates").and_then(serde_json::Value::as_bool) != Some(true) {
        violations.push(format!(
            "OpenAPI operation `{name}`: `x-campbooks.rate_limit` is only for operations that \
             mutate"
        ));
    }
}

/// An event type a module publishes, or one a consumer reads.
#[derive(Debug, Clone)]
pub struct EventDecl {
    /// The publishing module, or the consumer's name.
    pub by: String,
    /// `DomainEvent::NAME`.
    pub name: String,
    /// `DomainEvent::VERSION`.
    pub version: u16,
}

/// Every event a consumer handles is published (declared with
/// `Background::publishes`) by some module, at the version the consumer
/// reads, so the events catalogue lists every trigger in use.
pub fn check_events(published: &[EventDecl], consumed: &[EventDecl]) -> Vec<String> {
    let mut violations = Vec::new();
    for consumer in consumed {
        match published.iter().find(|p| p.name == consumer.name) {
            None => violations.push(format!(
                "event handler `{}` consumes `{}`, which no module declares with \
                 `Background::publishes`",
                consumer.by, consumer.name
            )),
            Some(declared) if declared.version != consumer.version => {
                violations.push(format!(
                    "event handler `{}` reads `{}` version {}, but `{}` publishes version {}",
                    consumer.by, consumer.name, consumer.version, declared.by, declared.version
                ));
            }
            Some(_) => {}
        }
    }
    violations
}

/// A scope as a module declares it.
#[derive(Debug, Clone)]
pub struct DeclaredScope {
    /// The declaring module.
    pub module: String,
    /// Wire name.
    pub name: String,
    /// What granting it allows.
    pub description: String,
}

static SCOPE_NAME: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^([a-z][a-z0-9_]*):([a-z][a-z0-9_]*)$")
        .unwrap_or_else(|e| unreachable!("static regex compiles: {e}"))
});

/// Scope names are `<module>:<action>` (lower-case), named after the module
/// that declares them, unique, and described.
pub fn check_scopes(scopes: &[DeclaredScope]) -> Vec<String> {
    let mut violations = Vec::new();
    let mut seen: BTreeMap<&str, &str> = BTreeMap::new();
    for scope in scopes {
        match SCOPE_NAME.captures(&scope.name) {
            None => violations.push(format!(
                "scope `{}` (module `{}`) must be `<module>:<action>`, lower-case",
                scope.name, scope.module
            )),
            Some(caps) if caps.get(1).map(|m| m.as_str()) != Some(scope.module.as_str()) => {
                violations.push(format!(
                    "scope `{}` is declared by module `{}`; a module declares only \
                     `{}:<action>` scopes",
                    scope.name, scope.module, scope.module
                ));
            }
            Some(_) => {}
        }
        if scope.description.trim().is_empty() {
            violations.push(format!("scope `{}` needs a description", scope.name));
        }
        if let Some(other) = seen.insert(&scope.name, &scope.module) {
            violations.push(format!(
                "scope `{}` is declared twice (modules `{other}` and `{}`)",
                scope.name, scope.module
            ));
        }
    }
    violations
}

// --- instance administration (ADR 0015) ------------------------------------

/// The module that owns admin profiles and the audit trail. It serves no
/// operation, and only it (and the server's CLI) may name its schema.
pub const ADMIN_MODULE: &str = "admin";

/// Path prefix of every admin operation; then the owning module's name.
pub const ADMIN_PATH_PREFIX: &str = "/admin/v1/";

/// The admin document's security scheme and the scope it requires.
pub const ADMIN_SCHEME: &str = "admin";

/// The scope every admin operation requires.
pub const ADMIN_SCOPE: &str = "admin:access";

/// The extension naming an admin operation's permission.
pub const ADMIN_EXTENSION: &str = "x-campbooks-admin";

/// The transaction setting that opens cross-workspace reads; only the
/// kernel may name it.
pub const ADMIN_READS_SETTING: &str = "campbooks.admin_reads";

/// The tooling crate (`campbooks-tracking-plan`): repository tooling with
/// no database access.
const TOOLING_DIR: &str = "crates/tracking-plan/";

/// Files besides the admin module allowed to name schema `admin`: the
/// server's CLI.
pub const ADMIN_CLI_FILES: [&str; 2] = [
    "crates/server/src/main.rs",
    "crates/server/src/admin_cli.rs",
];

/// The admin module serves nothing: `routes` is how many paths its API
/// router has and `admin_routes` its admin router's (`None`: it has none).
pub fn check_admin_module(routes: usize, admin_routes: Option<usize>) -> Vec<String> {
    let mut violations = Vec::new();
    if routes > 0 {
        violations.push(format!(
            "crates/modules/{ADMIN_MODULE}: the admin module serves no route ({routes} found); \
             admin profiles change only through `campbooks-server admin` (ADR 0015)"
        ));
    }
    if admin_routes.is_some() {
        violations.push(format!(
            "crates/modules/{ADMIN_MODULE}: the admin module has no admin router; admin \
             profiles are never read or written over HTTP (ADR 0015)"
        ));
    }
    violations
}

/// No operation of either document belongs to the admin module, and the
/// operation catalogue holds no admin path or admin metadata.
pub fn check_no_admin_owner(
    owners: &Owners,
    admin_owners: &Owners,
    catalogue: &serde_json::Value,
) -> Vec<String> {
    let mut violations = Vec::new();
    for ((method, path), owner) in owners.iter().chain(admin_owners) {
        if owner == ADMIN_MODULE {
            violations.push(format!(
                "operation `{method} {path}` is served by the admin module, which serves \
                 nothing: admin profiles are never read or written over HTTP (ADR 0015)"
            ));
        }
    }
    if let Some(paths) = catalogue
        .get("paths")
        .and_then(serde_json::Value::as_object)
    {
        for (path, item) in paths {
            for method in OPERATION_METHODS {
                let Some(operation) = item.get(method) else {
                    continue;
                };
                let name = format!("{} {path}", method.to_uppercase());
                if path.starts_with(ADMIN_PATH_PREFIX)
                    || path == ADMIN_PATH_PREFIX.trim_end_matches('/')
                {
                    violations.push(format!(
                        "OpenAPI operation `{name}` is an admin path in the operation catalogue; \
                         serve it from the module's `admin_router` (openapi/admin.json)"
                    ));
                }
                if operation.get(ADMIN_EXTENSION).is_some() {
                    violations.push(format!(
                        "OpenAPI operation `{name}` carries `{ADMIN_EXTENSION}` in the operation \
                         catalogue; admin operations belong to the admin document"
                    ));
                }
            }
        }
    }
    violations
}

/// Every operation of the admin document (`openapi/admin.json`): served by
/// a module's admin router, under `/admin/v1/<module>/`, with an
/// `operationId` `<module>.<verb>[_<noun>]` unique across both documents,
/// the `admin` security scheme with exactly `admin:access`, and an
/// `x-campbooks-admin` naming a known permission and whether it mutates
/// (matching the method), and no `x-campbooks` (it is not in the
/// catalogue).
pub fn check_admin_operations(
    spec: &serde_json::Value,
    owners: &Owners,
    permissions: &BTreeSet<String>,
    catalogue_ids: &BTreeSet<String>,
) -> Vec<String> {
    let mut violations = Vec::new();
    let Some(paths) = spec.get("paths").and_then(serde_json::Value::as_object) else {
        return violations;
    };
    let mut ids: BTreeMap<String, String> = BTreeMap::new();
    for (path, item) in paths {
        for method in OPERATION_METHODS {
            let Some(operation) = item.get(method) else {
                continue;
            };
            let method = method.to_uppercase();
            let name = format!("{method} {path}");
            let owner = owners.get(&(method.clone(), path.clone()));
            match owner {
                None => violations.push(format!(
                    "admin operation `{name}` is not served by any module's admin router"
                )),
                Some(owner) if !path.starts_with(&format!("{ADMIN_PATH_PREFIX}{owner}/")) => {
                    violations.push(format!(
                        "admin operation `{name}` must live under `{ADMIN_PATH_PREFIX}{owner}/`, \
                         the module that serves it"
                    ));
                }
                Some(_) => {}
            }
            match operation
                .get("operationId")
                .and_then(serde_json::Value::as_str)
            {
                None => violations.push(format!("admin operation `{name}` has no `operationId`")),
                Some(id) => {
                    match (OPERATION_ID.captures(id), owner) {
                        (None, _) => violations.push(format!(
                            "admin operation `{name}`: operationId `{id}` must be \
                             `<module>.<verb>[_<noun>]`, lower-case snake case"
                        )),
                        (Some(caps), Some(owner))
                            if caps.get(1).map(|m| m.as_str()) != Some(owner.as_str()) =>
                        {
                            violations.push(format!(
                                "admin operation `{name}`: operationId `{id}` must start with \
                                 `{owner}.`, the module that serves it"
                            ));
                        }
                        _ => {}
                    }
                    if catalogue_ids.contains(id) {
                        violations.push(format!(
                            "admin operation `{name}`: operationId `{id}` is also an operation \
                             of the catalogue; ids are unique across both documents"
                        ));
                    }
                    if let Some(other) = ids.insert(id.to_owned(), name.clone()) {
                        violations.push(format!(
                            "operationId `{id}` is used by both `{other}` and `{name}`"
                        ));
                    }
                }
            }
            let expected = serde_json::json!([{ ADMIN_SCHEME: [ADMIN_SCOPE] }]);
            if operation.get("security") != Some(&expected) {
                violations.push(format!(
                    "admin operation `{name}` must declare exactly \
                     `security((\"{ADMIN_SCHEME}\" = [\"{ADMIN_SCOPE}\"]))`"
                ));
            }
            if operation.get("x-campbooks").is_some() {
                violations.push(format!(
                    "admin operation `{name}` carries `x-campbooks`: admin operations are not \
                     part of the operation catalogue"
                ));
            }
            violations.extend(check_admin_meta(&name, &method, operation, permissions));
        }
    }
    violations
}

fn check_admin_meta(
    name: &str,
    method: &str,
    operation: &serde_json::Value,
    permissions: &BTreeSet<String>,
) -> Vec<String> {
    let mut violations = Vec::new();
    let Some(meta) = operation
        .get(ADMIN_EXTENSION)
        .and_then(serde_json::Value::as_object)
    else {
        return vec![format!(
            "admin operation `{name}` has no `{ADMIN_EXTENSION}` object: add \
             `extensions((\"{ADMIN_EXTENSION}\" = json!(AdminOperation::query::<P>())))`"
        )];
    };
    for key in meta.keys() {
        if key != "permission" && key != "mutates" {
            violations.push(format!(
                "admin operation `{name}`: unknown key `{ADMIN_EXTENSION}.{key}`"
            ));
        }
    }
    match meta.get("permission").and_then(serde_json::Value::as_str) {
        Some(permission) if permissions.contains(permission) => {}
        Some(permission) => violations.push(format!(
            "admin operation `{name}`: `{ADMIN_EXTENSION}.permission` `{permission}` is not a \
             permission (known: {})",
            permissions.iter().cloned().collect::<Vec<_>>().join(", ")
        )),
        None => violations.push(format!(
            "admin operation `{name}`: `{ADMIN_EXTENSION}.permission` is missing"
        )),
    }
    match meta.get("mutates").and_then(serde_json::Value::as_bool) {
        None => violations.push(format!(
            "admin operation `{name}`: `{ADMIN_EXTENSION}.mutates` must be a boolean"
        )),
        Some(true) if !matches!(method, "POST" | "PUT" | "PATCH" | "DELETE") => {
            violations.push(format!(
                "admin operation `{name}` mutates, so it must be POST, PUT, PATCH or DELETE"
            ));
        }
        Some(false) if !matches!(method, "GET" | "HEAD") => violations.push(format!(
            "admin operation `{name}` does not mutate, so it must be GET or HEAD"
        )),
        Some(_) => {}
    }
    violations
}

/// The Rust sources of the workspace crates, as `(path relative to the
/// root, text)`: only the admin module and the server's CLI name schema
/// `admin` (in SQL inside string literals), and only the kernel names the
/// `campbooks.admin_reads` setting (anywhere).
pub fn check_admin_sources(files: &[(String, String)]) -> Vec<String> {
    let mut violations = Vec::new();
    for (path, text) in files {
        let in_admin = path.starts_with(&format!("crates/modules/{ADMIN_MODULE}/"));
        let in_cli = ADMIN_CLI_FILES.contains(&path.as_str());
        // The tooling crate reads no database (it has no sqlx); it names the
        // admin API's OpenAPI file, `openapi/admin.json`.
        let in_tooling = path.starts_with(TOOLING_DIR);
        if !in_admin && !in_cli && !in_tooling {
            for literal in string_literals(text) {
                if names_schema(&literal, ADMIN_MODULE) {
                    violations.push(format!(
                        "{path}: names schema `{ADMIN_MODULE}`; only crates/modules/{ADMIN_MODULE} \
                         and the server's CLI may read or write admin profiles (ADR 0015)"
                    ));
                    break;
                }
            }
        }
        if !path.starts_with("crates/kernel/") && text.to_lowercase().contains(ADMIN_READS_SETTING)
        {
            violations.push(format!(
                "{path}: names `{ADMIN_READS_SETTING}`; only the kernel may (use \
                 `kernel::db::begin_admin`)"
            ));
        }
    }
    violations
}

/// Whether the SQL in `literal` names `schema`: a qualified name
/// `<schema>.<x>` or a `CREATE`/`ALTER`/`DROP SCHEMA <schema>` (comments and
/// quoted values ignored).
fn names_schema(literal: &str, schema: &str) -> bool {
    let sql = SQL_NOISE
        .replace_all(literal, " ")
        .to_lowercase()
        .replace('"', "");
    QUALIFIED_NAME
        .captures_iter(&sql)
        .chain(SCHEMA_STATEMENT.captures_iter(&sql))
        .any(|caps| caps.get(1).is_some_and(|m| m.as_str() == schema))
}

/// The contents of every string literal of the Rust source `text` (plain,
/// raw and byte strings), skipping comments and character literals.
pub fn string_literals(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut literals = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        match c {
            '/' if next == Some('/') => {
                while i < chars.len() && chars[i] != '\n' {
                    i += 1;
                }
            }
            '/' if next == Some('*') => {
                i += 2;
                while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '/') {
                    i += 1;
                }
                i += 2;
            }
            'r' | 'b' if is_raw_string_start(&chars, i) => {
                let mut j = i + 1;
                if chars.get(j) == Some(&'r') {
                    j += 1;
                }
                let mut hashes = 0;
                while chars.get(j) == Some(&'#') {
                    hashes += 1;
                    j += 1;
                }
                j += 1; // the opening quote
                let start = j;
                loop {
                    if j >= chars.len() {
                        break;
                    }
                    if chars[j] == '"' && (1..=hashes).all(|k| chars.get(j + k) == Some(&'#')) {
                        break;
                    }
                    j += 1;
                }
                literals.push(chars[start..j.min(chars.len())].iter().collect());
                i = j + 1 + hashes;
            }
            '"' => {
                let mut j = i + 1;
                let mut literal = String::new();
                while j < chars.len() && chars[j] != '"' {
                    if chars[j] == '\\' {
                        if let Some(escaped) = chars.get(j + 1) {
                            literal.push(*escaped);
                        }
                        j += 2;
                        continue;
                    }
                    literal.push(chars[j]);
                    j += 1;
                }
                literals.push(literal);
                i = j + 1;
            }
            '\'' => {
                // A character literal ('x', '\n', '\u{1F600}') or a lifetime.
                if next == Some('\\') {
                    let mut j = i + 3;
                    while j < chars.len() && chars[j] != '\'' {
                        j += 1;
                    }
                    i = j + 1;
                } else if chars.get(i + 2) == Some(&'\'') {
                    i += 3;
                } else {
                    i += 1;
                }
            }
            _ => i += 1,
        }
    }
    literals
}

/// Whether a raw (or byte raw / byte) string starts at `i`: `r"`, `r#`,
/// `br"`, `br#` or `b"`, not inside an identifier.
fn is_raw_string_start(chars: &[char], i: usize) -> bool {
    let preceded_by_ident = i > 0 && (chars[i - 1].is_alphanumeric() || chars[i - 1] == '_');
    if preceded_by_ident {
        return false;
    }
    match (chars.get(i), chars.get(i + 1), chars.get(i + 2)) {
        (Some('r'), Some('"' | '#'), _) | (Some('b'), Some('r'), Some('"' | '#')) => {
            // `r#ident` raw identifiers are not strings.
            let mut j = if chars[i] == 'b' { i + 2 } else { i + 1 };
            while chars.get(j) == Some(&'#') {
                j += 1;
            }
            chars.get(j) == Some(&'"')
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn krate(name: &str, path: &str, deps: &[&str]) -> Crate {
        Crate {
            name: name.into(),
            path: path.into(),
            version: "0.1.0".into(),
            deps: deps.iter().map(|d| (*d).to_owned()).collect(),
        }
    }

    fn healthy() -> Vec<Crate> {
        vec![
            krate("campbooks-kernel", "crates/kernel", &[]),
            krate("campbooks-ai", "crates/ai", &["campbooks-kernel"]),
            krate(
                "campbooks-mail",
                "crates/modules/mail",
                &["campbooks-kernel", "campbooks-ai"],
            ),
            krate(
                "campbooks-calendar",
                "crates/modules/calendar",
                &["campbooks-kernel"],
            ),
            krate(
                "campbooks-server",
                "crates/server",
                &["campbooks-kernel", "campbooks-mail", "campbooks-calendar"],
            ),
            krate("xtask", "xtask", &["campbooks-server"]),
        ]
    }

    fn allow(from: &str, to: &str) -> Allow {
        Allow {
            from: from.into(),
            to: to.into(),
            reason: "because".into(),
        }
    }

    #[test]
    fn classifies_paths() {
        assert_eq!(Kind::from_path("crates/kernel"), Kind::Kernel);
        assert_eq!(
            Kind::from_path("crates/modules/mail"),
            Kind::Module("mail".into())
        );
        assert_eq!(Kind::from_path("xtask"), Kind::Xtask);
        assert_eq!(Kind::from_path("crates/tracking-plan"), Kind::Tooling);
        assert_eq!(Kind::from_path("crates/utils"), Kind::Unknown);
    }

    #[test]
    fn tooling_depends_on_nothing_and_only_xtask_on_it() {
        let mut crates = healthy();
        crates.push(krate(
            "campbooks-tracking-plan",
            "crates/tracking-plan",
            &["campbooks-kernel"],
        ));
        crates[3].deps.push("campbooks-tracking-plan".into());
        let v = check_dependencies(&crates, &[]);
        assert_eq!(v.len(), 2, "{v:?}");
        assert!(
            v.iter()
                .any(|v| v.contains("only xtask may depend on the tooling"))
        );
        assert!(v.iter().any(|v| v.contains("tooling crates depend on no")));
        let xtask = crates.iter().position(|c| c.name == "xtask").unwrap();
        crates[xtask].deps.push("campbooks-tracking-plan".into());
        assert_eq!(check_dependencies(&crates, &[]).len(), 2);
    }

    #[test]
    fn healthy_graph_passes() {
        assert!(check_dependencies(&healthy(), &[]).is_empty());
    }

    #[test]
    fn kernel_depending_on_anything_fails() {
        let mut crates = healthy();
        crates[0].deps.push("campbooks-ai".into());
        let v = check_dependencies(&crates, &[]);
        assert_eq!(v.len(), 1, "{v:?}");
        assert!(v[0].contains("campbooks-kernel") && v[0].contains("kernel depends on no"));
    }

    #[test]
    fn ai_depending_on_a_module_fails() {
        let mut crates = healthy();
        crates[1].deps.push("campbooks-mail".into());
        let v = check_dependencies(&crates, &[]);
        assert_eq!(v.len(), 1, "{v:?}");
        assert!(v[0].contains("depends only on the kernel"));
    }

    #[test]
    fn module_to_module_needs_an_allow_entry() {
        let mut crates = healthy();
        crates[2].deps.push("campbooks-calendar".into());
        let v = check_dependencies(&crates, &[]);
        assert_eq!(v.len(), 1, "{v:?}");
        assert!(v[0].contains("modules don't import modules"));
        assert!(check_dependencies(&crates, &[allow("mail", "calendar")]).is_empty());
        // Direction matters.
        assert_eq!(
            check_dependencies(&crates, &[allow("calendar", "mail")]).len(),
            1
        );
    }

    #[test]
    fn only_xtask_may_depend_on_server() {
        let mut crates = healthy();
        crates[3].deps.push("campbooks-server".into());
        let v = check_dependencies(&crates, &[]);
        assert_eq!(v.len(), 1, "{v:?}");
        assert!(v[0].contains("only xtask may depend on the server"));
    }

    #[test]
    fn reports_every_violation_and_unknown_crates() {
        let mut crates = healthy();
        crates[0].deps.push("campbooks-ai".into());
        crates[1].deps.push("campbooks-server".into());
        crates.push(krate("helpers", "crates/helpers", &[]));
        assert_eq!(check_dependencies(&crates, &[]).len(), 3);
    }

    #[test]
    fn allowlist_must_name_modules_and_reason() {
        let modules: BTreeSet<String> = ["mail", "calendar"].map(String::from).into();
        assert!(check_allowlist(&[allow("mail", "calendar")], &modules).is_empty());

        let bad = Allow {
            from: "mail".into(),
            to: "money".into(),
            reason: " ".into(),
        };
        let v = check_allowlist(&[bad], &modules);
        assert_eq!(v.len(), 2, "{v:?}");
    }

    #[test]
    fn migration_names() {
        let modules: BTreeSet<String> = ["system", "mail_sync"].map(String::from).into();
        let names = [
            "20261012000000_system__init.sql",
            "20261012000001_mail_sync__add_cursor.sql",
            "20261012000002_billing__init.sql",
            "2026101200000_system__short_ts.sql",
            "20261012000003_system_init.sql",
            "20261012000004_System__init.sql",
            "20261012000000_system__dupe.sql",
            "README.md",
        ]
        .map(String::from);
        let v = check_migrations(&names, &modules);
        assert_eq!(v.len(), 6, "{v:#?}");
        assert!(v[0].contains("`billing` is not a module, `kernel` or `bff`"));
        assert!(v.iter().any(|m| m.contains("already used")));
    }

    #[test]
    fn infrastructure_owns_its_migrations_and_its_names() {
        let modules: BTreeSet<String> = ["system"].map(String::from).into();
        let names = [
            "20261012100000_kernel__events_and_jobs.sql".to_owned(),
            "20261013000000_bff__sessions.sql".to_owned(),
        ];
        assert!(check_migrations(&names, &modules).is_empty());

        let clash: BTreeSet<String> = ["kernel", "bff"].map(String::from).into();
        let v = check_migrations(&[], &clash);
        assert_eq!(v.len(), 2, "{v:?}");
        assert!(v.iter().all(|m| m.contains("reserved")));
    }

    #[test]
    fn modules_may_not_take_a_reserved_owner_name() {
        let modules: BTreeSet<String> = ["changes", "mail"].map(String::from).into();
        assert_eq!(check_reserved_owner(&modules, "changes").len(), 1);
        assert!(check_reserved_owner(&modules, "feed").is_empty());
    }

    #[test]
    fn bff_migrations_touch_only_bff() {
        let modules: BTreeSet<String> = ["system"].map(String::from).into();
        let ok = [sql(
            "20261013000000_bff__sessions.sql",
            "CREATE SCHEMA bff; CREATE TABLE bff.sessions (id uuid);",
        )];
        assert!(check_migration_schemas(&ok, &modules).is_empty());
        let bad = [
            sql(
                "20261013000001_bff__oops.sql",
                "CREATE TABLE bff.s (id uuid DEFAULT kernel.uuid_v7());",
            ),
            sql(
                "20261013000002_system__oops.sql",
                "DROP TABLE bff.sessions;",
            ),
        ];
        let v = check_migration_schemas(&bad, &modules);
        assert_eq!(v.len(), 2, "{v:#?}");
        assert!(v[0].contains("touches schema `kernel`"));
        assert!(v[1].contains("touches schema `bff`"));
    }

    fn sql(name: &str, body: &str) -> (String, String) {
        (name.to_owned(), body.to_owned())
    }

    #[test]
    fn migrations_touch_only_their_owner_schema() {
        let modules: BTreeSet<String> = ["system", "mail"].map(String::from).into();
        let ok = [
            sql(
                "20261012000000_system__init.sql",
                "-- kernel.jobs is not ours\nCREATE SCHEMA IF NOT EXISTS system;\n\
                 CREATE TABLE system.instance (id uuid, note text DEFAULT 'mail.x');\n\
                 SELECT i.id FROM system.instance i;",
            ),
            sql(
                "20261012100000_kernel__init.sql",
                "CREATE SCHEMA kernel; CREATE TABLE kernel.jobs (id uuid DEFAULT kernel.uuid_v7());\n\
                 /* system.instance */ SELECT j.id FROM kernel.jobs j;",
            ),
        ];
        assert!(check_migration_schemas(&ok, &modules).is_empty());

        let bad = [
            sql(
                "20261012000001_mail__init.sql",
                "CREATE TABLE mail.m (id uuid DEFAULT kernel.uuid_v7());\n\
                 INSERT INTO \"system\".instance VALUES (gen_random_uuid());",
            ),
            sql("20261012000002_mail__more.sql", "CREATE SCHEMA billing;"),
            sql(
                "20261012000003_kernel__oops.sql",
                "ALTER TABLE mail.m ADD COLUMN x int; SET search_path = mail;",
            ),
        ];
        let v = check_migration_schemas(&bad, &modules);
        assert_eq!(v.len(), 5, "{v:#?}");
        assert!(v[0].contains("touches schema `kernel`"));
        assert!(v[1].contains("touches schema `system`"));
        assert!(v[2].contains("touches schema `billing`"));
        assert!(v[3].contains("touches schema `mail`"));
        assert!(v[4].contains("search_path"));
    }

    #[test]
    fn only_a_functions_pinned_search_path_is_allowed() {
        let modules: BTreeSet<String> = ["mail"].map(String::from).into();
        let ok = [sql(
            "20261012000000_kernel__count.sql",
            "CREATE FUNCTION kernel.count(n integer) RETURNS bigint LANGUAGE sql\n\
             SECURITY DEFINER\n    SET search_path = pg_catalog, pg_temp\n\
             AS $$ SELECT count(*) FROM kernel.jobs $$;",
        )];
        assert!(check_migration_schemas(&ok, &modules).is_empty());
        let bad = [
            sql(
                "20261012000001_kernel__path.sql",
                "SET search_path = pg_catalog, pg_temp;",
            ),
            sql(
                "20261012000002_kernel__path.sql",
                "CREATE FUNCTION kernel.f() RETURNS int LANGUAGE sql \
                 SET search_path = kernel AS $$ SELECT 1 $$;",
            ),
            sql(
                "20261012000003_kernel__path.sql",
                "CREATE FUNCTION kernel.f() RETURNS int LANGUAGE sql \
                 SET search_path = pg_catalog, pg_temp AS $$ SELECT 1 $$; \
                 SET search_path TO kernel;",
            ),
            // Another entry after `pg_temp`, so it is not last.
            sql(
                "20261012000004_kernel__path.sql",
                "CREATE FUNCTION kernel.f() RETURNS int LANGUAGE sql SECURITY DEFINER \
                 SET search_path = pg_catalog, pg_temp, kernel AS $$ SELECT 1 $$;",
            ),
            sql(
                "20261012000005_kernel__path.sql",
                "CREATE FUNCTION kernel.f() RETURNS int LANGUAGE sql SECURITY DEFINER \
                 SET search_path = pg_catalog, pg_temp , public AS $$ SELECT 1 $$;",
            ),
            // `pg_temp` before another schema.
            sql(
                "20261012000006_kernel__path.sql",
                "CREATE FUNCTION kernel.f() RETURNS int LANGUAGE sql SECURITY DEFINER \
                 SET search_path = pg_temp, pg_catalog AS $$ SELECT 1 $$;",
            ),
            // A session-wide statement at the very end of the file.
            sql(
                "20261012000007_kernel__path.sql",
                "SELECT 1;\nSET search_path = pg_catalog, pg_temp\n",
            ),
            // `pg_temp$x` and `pg_tempx` are other schemas, not `pg_temp`
            // (campbooks-cloud#214).
            sql(
                "20261012000008_kernel__path.sql",
                "CREATE FUNCTION kernel.f() RETURNS int LANGUAGE sql SECURITY DEFINER \
                 SET search_path = pg_catalog, pg_temp$x AS $$ SELECT 1 $$;",
            ),
            sql(
                "20261012000009_kernel__path.sql",
                "CREATE FUNCTION kernel.f() RETURNS int LANGUAGE sql SECURITY DEFINER \
                 SET search_path = pg_catalog, pg_tempx AS $$ SELECT 1 $$;",
            ),
            // The path changed at run time, through a string.
            sql(
                "20261012000010_kernel__path.sql",
                "CREATE FUNCTION kernel.f() RETURNS int LANGUAGE plpgsql SECURITY DEFINER \
                 SET search_path = pg_catalog, pg_temp AS $$ BEGIN \
                 PERFORM set_config('search_path', 'kernel', true); RETURN 1; END $$;",
            ),
            sql(
                "20261012000011_kernel__path.sql",
                "SELECT pg_catalog.set_config('SEARCH_PATH', 'public', false);",
            ),
        ];
        let v = check_migration_schemas(&bad, &modules);
        assert_eq!(v.len(), 11, "{v:#?}");
        assert!(v.iter().all(|v| v.contains("search_path")), "{v:#?}");
    }

    #[test]
    fn background_errors_are_violations() {
        assert!(check_background(&[]).is_empty());
        let v = check_background(&["job `x`: must be `mail.<name>`".to_owned()]);
        assert_eq!(
            v,
            ["background registration: job `x`: must be `mail.<name>`"]
        );
    }

    #[test]
    fn versions_must_match_version_file() {
        let crates = healthy();
        assert!(check_versions("0.1.0\n", &crates).is_empty());
        assert_eq!(check_versions("0.2.0", &crates).len(), crates.len());
    }

    fn catalogue() -> BTreeSet<String> {
        ["mail:read".to_owned()].into()
    }

    const SCHEMES: Schemes<'static> = Schemes {
        bearer: "bearer",
        session: "session",
    };

    #[test]
    fn explicit_security_passes() {
        let spec = serde_json::json!({ "paths": {
            "/api/v1/system/health": { "get": { "security": [{}] } },
            "/api/v1/mail/threads": {
                "get": { "security": [{ "bearer": ["mail:read"] }] },
                "parameters": [],
            },
        }});
        assert!(check_operation_security(&spec, SCHEMES, &catalogue()).is_empty());
        let public_by_empty_list =
            serde_json::json!({ "paths": { "/x": { "get": { "security": [] } } } });
        assert!(check_operation_security(&public_by_empty_list, SCHEMES, &catalogue()).is_empty());
    }

    #[test]
    fn operation_without_security_fails() {
        let spec = serde_json::json!({ "paths": {
            "/api/v1/mail/threads": { "get": {}, "post": { "security": [{}] } },
        }});
        let v = check_operation_security(&spec, SCHEMES, &catalogue());
        assert_eq!(v.len(), 1, "{v:?}");
        assert!(v[0].contains("`GET /api/v1/mail/threads` declares no `security`"));
    }

    #[test]
    fn scoped_security_must_name_a_known_scope_of_the_bearer_scheme() {
        let spec = serde_json::json!({ "paths": { "/x": {
            "get": { "security": [{ "bearer": [] }] },
            "put": { "security": [{ "bearer": ["mail:write"] }] },
            "post": { "security": [{ "apiKey": ["mail:read"] }] },
            "delete": { "security": ["bearer"] },
        }}});
        let v = check_operation_security(&spec, SCHEMES, &catalogue());
        assert_eq!(v.len(), 4, "{v:#?}");
        assert!(v.iter().any(|m| m.contains("but no scope")));
        assert!(
            v.iter()
                .any(|m| m.contains("`mail:write`, which no module declares"))
        );
        assert!(v.iter().any(|m| m.contains("scheme `apiKey`")));
        assert!(v.iter().any(|m| m.contains("malformed")));
    }

    #[test]
    fn the_session_scheme_is_for_scope_less_bff_operations_only() {
        let spec = serde_json::json!({ "paths": {
            "/bff/session": { "get": { "security": [{}, { "session": [] }] } },
            "/bff/api/{path}": { "post": { "security": [{ "session": [] }] } },
        }});
        assert!(check_operation_security(&spec, SCHEMES, &catalogue()).is_empty());

        let spec = serde_json::json!({ "paths": {
            "/api/v1/mail/threads": { "get": { "security": [{ "session": [] }] } },
            "/bff/logout": { "post": { "security": [{ "session": ["mail:read"] }] } },
        }});
        let v = check_operation_security(&spec, SCHEMES, &catalogue());
        assert_eq!(v.len(), 2, "{v:#?}");
        assert!(v.iter().any(|m| m.contains("only the web BFF")));
        assert!(v.iter().any(|m| m.contains("takes no scopes")));
    }

    fn meta(mutates: bool) -> serde_json::Value {
        serde_json::json!({
            "mutates": mutates, "automation": true, "assistant": false,
            "risk": "low", "records": ["mail.thread"],
        })
    }

    fn owners(routes: &[(&str, &str, &str)]) -> Owners {
        routes
            .iter()
            .map(|(method, path, module)| {
                (
                    ((*method).to_owned(), (*path).to_owned()),
                    (*module).to_owned(),
                )
            })
            .collect()
    }

    fn healthy_catalogue() -> (serde_json::Value, Owners) {
        let mut archive = meta(true);
        archive["undo"] = serde_json::json!("mail.unarchive");
        let spec = serde_json::json!({ "paths": {
            "/api/v1/mail/threads": {
                "get": { "operationId": "mail.list_threads", "x-campbooks": meta(false) },
                "parameters": [],
            },
            "/api/v1/mail/threads/{id}/archive": {
                "post": { "operationId": "mail.archive", "x-campbooks": archive },
            },
            "/api/v1/mail/threads/{id}/unarchive": {
                "post": { "operationId": "mail.unarchive", "x-campbooks": meta(true) },
            },
        }});
        let owners = owners(&[
            ("GET", "/api/v1/mail/threads", "mail"),
            ("POST", "/api/v1/mail/threads/{id}/archive", "mail"),
            ("POST", "/api/v1/mail/threads/{id}/unarchive", "mail"),
        ]);
        (spec, owners)
    }

    #[test]
    fn complete_catalogue_entries_pass() {
        let (spec, owners) = healthy_catalogue();
        assert!(
            check_operations(&spec, &owners).is_empty(),
            "{:#?}",
            check_operations(&spec, &owners)
        );
    }

    #[test]
    fn operation_ids_are_present_prefixed_by_their_module_and_unique() {
        let spec = serde_json::json!({ "paths": {
            "/a": { "get": { "x-campbooks": meta(false) } },
            "/b": { "get": { "operationId": "calendar.list", "x-campbooks": meta(false) } },
            "/c": { "get": { "operationId": "mail_list", "x-campbooks": meta(false) } },
            "/d": { "get": { "operationId": "mail.list", "x-campbooks": meta(false) } },
            "/e": { "get": { "operationId": "mail.list", "x-campbooks": meta(false) } },
            "/f": { "get": { "operationId": "mail.orphan", "x-campbooks": meta(false) } },
        }});
        let owners = owners(&[
            ("GET", "/a", "mail"),
            ("GET", "/b", "mail"),
            ("GET", "/c", "mail"),
            ("GET", "/d", "mail"),
            ("GET", "/e", "mail"),
        ]);
        let v = check_operations(&spec, &owners);
        assert_eq!(v.len(), 5, "{v:#?}");
        assert!(
            v.iter()
                .any(|m| m.contains("`GET /a` has no `operationId`"))
        );
        assert!(
            v.iter()
                .any(|m| m.contains("`calendar.list` must start with `mail.`"))
        );
        assert!(
            v.iter()
                .any(|m| m.contains("`mail_list` must be `<module>.<verb>"))
        );
        assert!(v.iter().any(|m| m.contains("`mail.list` is used by both")));
        assert!(
            v.iter()
                .any(|m| m.contains("`GET /f` is not served by any module"))
        );
    }

    #[test]
    fn a_high_risk_operation_is_no_callers_but_the_persons() {
        let mut send = meta(true);
        send["risk"] = serde_json::json!("high");
        send["automation"] = serde_json::json!(false);
        let (mut spec, mut owners) = healthy_catalogue();
        spec["paths"]["/api/v1/mail/drafts/{id}/send"] =
            serde_json::json!({ "post": { "operationId": "mail.send", "x-campbooks": send } });
        owners.insert(
            (
                "POST".to_owned(),
                "/api/v1/mail/drafts/{id}/send".to_owned(),
            ),
            "mail".to_owned(),
        );
        assert!(
            check_operations(&spec, &owners).is_empty(),
            "{:#?}",
            check_operations(&spec, &owners)
        );
        for caller in ["automation", "assistant"] {
            let mut opted = spec.clone();
            opted["paths"]["/api/v1/mail/drafts/{id}/send"]["post"]["x-campbooks"][caller] =
                serde_json::json!(true);
            let v = check_operations(&opted, &owners);
            assert_eq!(v.len(), 1, "{v:#?}");
            assert!(
                v[0].contains("`POST /api/v1/mail/drafts/{id}/send` is high-risk")
                    && v[0].contains(&format!("`x-campbooks.{caller}`")),
                "{v:#?}"
            );
        }
    }

    #[test]
    fn x_campbooks_must_be_complete_and_well_typed() {
        let spec = serde_json::json!({ "paths": {
            "/a": { "get": { "operationId": "mail.a" } },
            "/b": { "get": { "operationId": "mail.b", "x-campbooks": { "mutates": false } } },
            "/c": { "get": { "operationId": "mail.c", "x-campbooks": {
                "mutates": "no", "automation": 1, "assistant": false, "risk": "medium",
                "records": ["thread", "calendar.event"], "colour": "red", "undo": 5,
            }}},
        }});
        let owners = owners(&[
            ("GET", "/a", "mail"),
            ("GET", "/b", "mail"),
            ("GET", "/c", "mail"),
        ]);
        let v = check_operations(&spec, &owners);
        assert!(
            v.iter()
                .any(|m| m.contains("`GET /a` has no `x-campbooks`")),
            "{v:#?}"
        );
        for key in ["automation", "assistant", "risk", "records"] {
            assert!(
                v.iter().any(|m| m.contains("`GET /b`")
                    && m.contains(&format!("`x-campbooks.{key}` is missing"))),
                "{key}: {v:#?}"
            );
        }
        for expected in [
            "`x-campbooks.mutates` must be a boolean",
            "`x-campbooks.automation` must be a boolean",
            "`x-campbooks.risk` must be \"low\" or \"high\"",
            "record type `thread` must be `<module>.<record>`",
            "record type `calendar.event` belongs to `calendar`",
            "unknown key `x-campbooks.colour`",
            "`x-campbooks.undo` must be an operationId",
        ] {
            assert!(v.iter().any(|m| m.contains(expected)), "{expected}: {v:#?}");
        }
    }

    #[test]
    fn mutates_matches_the_method() {
        let spec = serde_json::json!({ "paths": {
            "/a": {
                "get": { "operationId": "mail.a", "x-campbooks": meta(true) },
                "delete": { "operationId": "mail.b", "x-campbooks": meta(false) },
                "patch": { "operationId": "mail.c", "x-campbooks": meta(true) },
                "head": { "operationId": "mail.d", "x-campbooks": meta(false) },
            },
        }});
        let owners = owners(&[
            ("GET", "/a", "mail"),
            ("DELETE", "/a", "mail"),
            ("PATCH", "/a", "mail"),
            ("HEAD", "/a", "mail"),
        ]);
        let v = check_operations(&spec, &owners);
        assert_eq!(v.len(), 2, "{v:#?}");
        assert!(
            v.iter()
                .any(|m| m.contains("`GET /a` mutates, so it must be POST"))
        );
        assert!(
            v.iter()
                .any(|m| m.contains("`DELETE /a` does not mutate, so it must be GET"))
        );
    }

    #[test]
    fn a_body_query_is_a_post_that_does_not_mutate() {
        let mut body_query = meta(false);
        body_query["query_in"] = serde_json::json!("body");
        let mut wrong = meta(true);
        wrong["query_in"] = serde_json::json!("body");
        let spec = serde_json::json!({ "paths": {
            "/q": { "post": { "operationId": "mail.q", "x-campbooks": body_query } },
            "/w": { "post": { "operationId": "mail.w", "x-campbooks": wrong } },
            "/p": { "post": { "operationId": "mail.p", "x-campbooks": meta(false) } },
        }});
        let owners = owners(&[
            ("POST", "/q", "mail"),
            ("POST", "/w", "mail"),
            ("POST", "/p", "mail"),
        ]);
        let v = check_operations(&spec, &owners);
        assert_eq!(v.len(), 2, "{v:#?}");
        assert!(
            v.iter()
                .any(|m| m.contains("`POST /w`: `x-campbooks.query_in` is for reads"))
        );
        assert!(v.iter().any(|m| m.contains("`POST /p` does not mutate")));
    }

    #[test]
    fn a_rate_limit_class_is_send_on_a_mutating_operation() {
        let mut send = meta(true);
        send["rate_limit"] = serde_json::json!("send");
        let mut on_a_read = meta(false);
        on_a_read["rate_limit"] = serde_json::json!("send");
        let mut looser = meta(true);
        looser["rate_limit"] = serde_json::json!("read");
        let spec = serde_json::json!({ "paths": {
            "/s": { "post": { "operationId": "mail.s", "x-campbooks": send } },
            "/r": { "get": { "operationId": "mail.r", "x-campbooks": on_a_read } },
            "/l": { "post": { "operationId": "mail.l", "x-campbooks": looser } },
        }});
        let owners = owners(&[
            ("POST", "/s", "mail"),
            ("GET", "/r", "mail"),
            ("POST", "/l", "mail"),
        ]);
        let v = check_operations(&spec, &owners);
        assert_eq!(v.len(), 2, "{v:#?}");
        assert!(v.iter().any(|m| {
            m.contains("`GET /r`: `x-campbooks.rate_limit` is only for operations that mutate")
        }));
        assert!(
            v.iter()
                .any(|m| m.contains("`POST /l`: `x-campbooks.rate_limit` must be \"send\""))
        );
    }

    #[test]
    fn secret_once_is_a_boolean_on_a_mutating_operation() {
        let mut secret = meta(true);
        secret["secret_once"] = serde_json::json!(true);
        let mut on_a_read = meta(false);
        on_a_read["secret_once"] = serde_json::json!(true);
        let mut not_a_flag = meta(true);
        not_a_flag["secret_once"] = serde_json::json!("yes");
        let spec = serde_json::json!({ "paths": {
            "/s": { "post": { "operationId": "mail.s", "x-campbooks": secret } },
            "/r": { "get": { "operationId": "mail.r", "x-campbooks": on_a_read } },
            "/f": { "post": { "operationId": "mail.f", "x-campbooks": not_a_flag } },
        }});
        let owners = owners(&[
            ("POST", "/s", "mail"),
            ("GET", "/r", "mail"),
            ("POST", "/f", "mail"),
        ]);
        let v = check_operations(&spec, &owners);
        assert_eq!(v.len(), 2, "{v:#?}");
        assert!(v.iter().any(|m| {
            m.contains("`GET /r`: `x-campbooks.secret_once` is only for operations that mutate")
        }));
        assert!(
            v.iter()
                .any(|m| m.contains("`POST /f`: `x-campbooks.secret_once` must be a boolean"))
        );
    }

    #[test]
    fn a_guarded_answer_field_needs_secret_once_or_a_reason() {
        let mut issues = meta(true);
        issues["secret_once"] = serde_json::json!(true);
        let spec = serde_json::json!({
            "paths": {
                "/leak": { "post": {
                    "operationId": "mail.leak", "x-campbooks": meta(true),
                    "responses": {
                        "201": { "content": { "application/json": {
                            "schema": { "$ref": "#/components/schemas/Leak" } } } },
                        "303": { "content": { "application/json": { "schema": {
                            "type": "object",
                            "properties": { "accept_url": { "type": "string" } } } } } },
                        "202": { "content": { "application/json": { "schema": {
                            "type": "object",
                            "properties": {
                                "clientSecret": { "type": "string" },
                                "accessToken": { "type": "string" },
                                "api_key": { "type": "string" },
                                "PrivateKey": { "type": "string" },
                                "invite_link": { "type": "string" },
                                "code": { "type": "string" },
                                "codec": { "type": "string" },
                                "keyboard": { "type": "string" },
                                "key_id": { "type": "string" },
                                "linked_at": { "type": "string" } } } } } },
                        "400": { "content": { "application/json": { "schema": {
                            "type": "object",
                            "properties": { "password": { "type": "string" } } } } } }
                    } } },
                "/issue": { "post": {
                    "operationId": "mail.issue", "x-campbooks": issues,
                    "responses": { "201": { "content": { "application/json": {
                        "schema": { "$ref": "#/components/schemas/Leak" } } } } } } },
                "/read": { "get": {
                    "operationId": "mail.read", "x-campbooks": meta(false),
                    "responses": { "200": { "content": { "application/json": {
                        "schema": { "$ref": "#/components/schemas/Leak" } } } } } } },
                "/tagged": { "post": {
                    "operationId": "mail.tagged", "x-campbooks": meta(true),
                    "responses": { "200": { "content": { "application/json": { "schema": {
                        "type": "object",
                        "properties": { "content_token": { "type": "string" } } } } } } } } }
            },
            "components": { "schemas": {
                "Leak": { "allOf": [ { "$ref": "#/components/schemas/Inner" },
                    { "type": "object", "properties": {
                        "tokens": { "type": "array", "items": { "type": "object",
                            "properties": { "refresh_token": { "type": "string" } } } } } } ] },
                "Inner": { "type": "object", "properties": {
                    "client_secret": { "type": "string" },
                    "secretary": { "type": "string" },
                    "tokenizer": { "type": "string" },
                    "next": { "$ref": "#/components/schemas/Leak" } } }
            } }
        });
        let allowed = [
            ("mail.tagged", "content_token", "a version tag"),
            ("mail.gone", "content_token", "stale"),
        ];
        let v = check_answer_field_names(&spec, &allowed);
        assert_eq!(v.len(), 12, "{v:#?}");
        for field in [
            "client_secret",
            "tokens",
            "refresh_token",
            "accept_url",
            "Location",
            "clientSecret",
            "accessToken",
            "api_key",
            "PrivateKey",
            "invite_link",
            "code",
        ] {
            assert!(
                v.iter()
                    .any(|m| m.contains(&format!("`mail.leak` answers `{field}`"))),
                "{field}: {v:#?}"
            );
        }
        assert!(
            v.iter()
                .any(|m| m.contains("lists `content_token` of `mail.gone`"))
        );
        // An error answer, a read, `secret_once`, a listed field and names
        // that only start like a secret are not reported.
        for quiet in [
            "password",
            "mail.issue",
            "mail.read",
            "mail.tagged",
            "secretary",
            "tokenizer",
            "codec",
            "keyboard",
            "key_id",
            "linked_at",
        ] {
            assert!(!v.iter().any(|m| m.contains(quiet)), "{quiet}: {v:#?}");
        }
    }

    #[test]
    fn field_names_split_into_words_in_any_case() {
        assert_eq!(name_words("clientSecret"), ["client", "secret"]);
        assert_eq!(name_words("refresh_token"), ["refresh", "token"]);
        assert_eq!(name_words("X-Api-Key"), ["x", "api", "key"]);
        assert_eq!(name_words("ID2Token"), ["id", "2", "token"]);
        assert_eq!(name_words("token2"), ["token", "2"]);
        for (name, words) in [
            ("IDToken", ["id", "token"]),
            ("APIToken", ["api", "token"]),
            ("OTPCode", ["otp", "code"]),
            ("JWTSecret", ["jwt", "secret"]),
            ("TOTPSecret", ["totp", "secret"]),
            ("SSOUrl", ["sso", "url"]),
            ("user.name", ["user", "name"]),
            ("user name", ["user", "name"]),
        ] {
            assert_eq!(name_words(name), words, "{name}");
            assert!(
                name == "user.name" || name == "user name" || guarded_field(name),
                "{name}"
            );
        }
        assert!(guarded_field("signupCodes") && guarded_field("REDIRECT_URI"));
        for name in [
            "apikey",
            "apikeys",
            "otp",
            "otps",
            "token2",
            "credentials",
            "passphrase",
            "public_key",
            "signingKeys",
        ] {
            assert!(guarded_field(name), "{name}");
        }
        assert!(!guarded_field("uriTemplate") && !guarded_field("links_count"));
        assert!(!guarded_field("key_id") && !guarded_field("keyboard"));
    }

    #[test]
    fn undo_targets_exist_and_mutate() {
        let (mut spec, owners) = healthy_catalogue();
        let ops = &mut spec["paths"];
        ops["/api/v1/mail/threads/{id}/archive"]["post"]["x-campbooks"]["undo"] =
            serde_json::json!("mail.restore");
        ops["/api/v1/mail/threads/{id}/unarchive"]["post"]["x-campbooks"]["undo"] =
            serde_json::json!("mail.list_threads");
        ops["/api/v1/mail/threads"]["get"]["x-campbooks"]["undo"] =
            serde_json::json!("mail.unarchive");
        let v = check_operations(&spec, &owners);
        assert_eq!(v.len(), 3, "{v:#?}");
        assert!(
            v.iter()
                .any(|m| m.contains("undo target `mail.restore` is not an operation"))
        );
        assert!(
            v.iter()
                .any(|m| m.contains("undo target `mail.list_threads` does not mutate"))
        );
        assert!(
            v.iter()
                .any(|m| m.contains("`mail.list_threads` names an undo"))
        );
    }

    fn event(by: &str, name: &str, version: u16) -> EventDecl {
        EventDecl {
            by: by.into(),
            name: name.into(),
            version,
        }
    }

    #[test]
    fn consumed_events_are_published_at_the_same_version() {
        let published = [event("mail", "mail.received", 2)];
        assert!(check_events(&published, &[event("parties.track", "mail.received", 2)]).is_empty());
        let v = check_events(
            &published,
            &[
                event("parties.track", "mail.received", 1),
                event("parties.other", "calendar.moved", 1),
            ],
        );
        assert_eq!(v.len(), 2, "{v:#?}");
        assert!(v[0].contains("reads `mail.received` version 1, but `mail` publishes version 2"));
        assert!(v[1].contains("`calendar.moved`, which no module declares"));
    }

    fn scope(module: &str, name: &str) -> DeclaredScope {
        DeclaredScope {
            module: module.into(),
            name: name.into(),
            description: "Does things.".into(),
        }
    }

    #[test]
    fn scopes_are_module_prefixed_unique_and_described() {
        assert!(
            check_scopes(&[
                scope("system", "system:read"),
                scope("mail_sync", "mail_sync:run")
            ])
            .is_empty()
        );

        let mut undescribed = scope("mail", "mail:send");
        undescribed.description = " ".into();
        let v = check_scopes(&[
            scope("mail", "calendar:read"),
            scope("mail", "Mail:Read"),
            scope("mail", "mail"),
            undescribed,
            scope("mail", "mail:read"),
            scope("mail", "mail:read"),
        ]);
        assert_eq!(v.len(), 5, "{v:#?}");
        assert!(v[0].contains("declared by module `mail`"));
        assert!(v.iter().any(|m| m.contains("needs a description")));
        assert!(v.iter().any(|m| m.contains("declared twice")));
    }

    // --- ADR 0015 -----------------------------------------------------------

    #[test]
    fn the_admin_module_serves_nothing() {
        assert!(check_admin_module(0, None).is_empty());
        let v = check_admin_module(2, Some(1));
        assert_eq!(v.len(), 2, "{v:#?}");
        assert!(v[0].contains("serves no route (2 found)"));
        assert!(v[1].contains("has no admin router"));
    }

    #[test]
    fn no_operation_belongs_to_the_admin_module() {
        let main = owners(&[("GET", "/api/v1/mail/threads", "mail")]);
        let admin = owners(&[("GET", "/admin/v1/system/overview", "system")]);
        let clean = serde_json::json!({ "paths": { "/api/v1/mail/threads": { "get": {} } } });
        assert!(check_no_admin_owner(&main, &admin, &clean).is_empty());

        let main = owners(&[("GET", "/api/v1/admin/profiles", "admin")]);
        let admin = owners(&[("POST", "/admin/v1/admin/profiles", "admin")]);
        let leaky = serde_json::json!({ "paths": {
            "/admin/v1/system/overview": { "get": {} },
            "/api/v1/mail/x": { "get": { "x-campbooks-admin": { "permission": "super" } } },
        }});
        let v = check_no_admin_owner(&main, &admin, &leaky);
        assert_eq!(v.len(), 4, "{v:#?}");
        assert!(
            v.iter()
                .any(|m| m.contains("`GET /api/v1/admin/profiles` is served by the admin module"))
        );
        assert!(
            v.iter().any(
                |m| m.contains("`POST /admin/v1/admin/profiles` is served by the admin module")
            )
        );
        assert!(
            v.iter()
                .any(|m| m.contains("admin path in the operation catalogue"))
        );
        assert!(
            v.iter()
                .any(|m| m.contains("carries `x-campbooks-admin` in the operation catalogue"))
        );
    }

    fn admin_op(permission: &str, mutates: bool) -> serde_json::Value {
        serde_json::json!({
            "operationId": "system.admin_overview",
            "security": [{ "admin": ["admin:access"] }],
            "x-campbooks-admin": { "permission": permission, "mutates": mutates },
        })
    }

    fn permissions() -> BTreeSet<String> {
        ["super", "system.read"].map(String::from).into()
    }

    #[test]
    fn well_formed_admin_operations_pass() {
        let spec = serde_json::json!({ "paths": {
            "/admin/v1/system/overview": { "get": admin_op("system.read", false) },
        }});
        let owners = owners(&[("GET", "/admin/v1/system/overview", "system")]);
        let v = check_admin_operations(&spec, &owners, &permissions(), &BTreeSet::new());
        assert!(v.is_empty(), "{v:#?}");
    }

    #[test]
    fn admin_operations_need_their_prefix_owner_permission_and_security() {
        let mut wrong_owner = admin_op("system.read", false);
        wrong_owner["operationId"] = serde_json::json!("jobs.overview");
        let mut public = admin_op("system.read", false);
        public["operationId"] = serde_json::json!("system.public_thing");
        public["security"] = serde_json::json!([{}]);
        let mut bearer = admin_op("system.read", false);
        bearer["operationId"] = serde_json::json!("system.bearer_thing");
        bearer["security"] = serde_json::json!([{ "bearer": ["system:read"] }]);
        let mut unknown = admin_op("root", false);
        unknown["operationId"] = serde_json::json!("system.unknown_permission");
        let mut bare = admin_op("system.read", false);
        bare["operationId"] = serde_json::json!("system.bare");
        bare.as_object_mut().unwrap().remove("x-campbooks-admin");
        bare["x-campbooks"] = serde_json::json!({});
        let mut posting = admin_op("super", false);
        posting["operationId"] = serde_json::json!("system.restart");
        let mut catalogued = admin_op("super", false);
        catalogued["operationId"] = serde_json::json!("system.version");
        let spec = serde_json::json!({ "paths": {
            "/admin/v1/system/a": { "get": wrong_owner },
            "/admin/v1/system/b": { "get": public },
            "/admin/v1/system/c": { "get": bearer },
            "/admin/v1/system/d": { "get": unknown },
            "/admin/v1/system/e": { "get": bare },
            "/admin/v1/system/f": { "post": posting },
            "/admin/v1/system/g": { "get": catalogued },
            "/admin/v1/jobs/h": { "get": admin_op("system.read", false) },
            "/admin/v1/system/i": { "get": admin_op("system.read", false) },
            "/api/v1/system/j": { "get": { "operationId": "system.misplaced",
                "security": [{ "admin": ["admin:access"] }],
                "x-campbooks-admin": { "permission": "super", "mutates": false } } },
        }});
        let owners = owners(&[
            ("GET", "/admin/v1/system/a", "system"),
            ("GET", "/admin/v1/system/b", "system"),
            ("GET", "/admin/v1/system/c", "system"),
            ("GET", "/admin/v1/system/d", "system"),
            ("GET", "/admin/v1/system/e", "system"),
            ("POST", "/admin/v1/system/f", "system"),
            ("GET", "/admin/v1/system/g", "system"),
            ("GET", "/admin/v1/jobs/h", "system"),
            ("GET", "/api/v1/system/j", "system"),
        ]);
        let catalogue: BTreeSet<String> = ["system.version".to_owned()].into();
        let v = check_admin_operations(&spec, &owners, &permissions(), &catalogue);
        for expected in [
            "`jobs.overview` must start with `system.`",
            "`GET /admin/v1/system/b` must declare exactly `security((\"admin\" = [\"admin:access\"]))`",
            "`GET /admin/v1/system/c` must declare exactly",
            "`root` is not a permission",
            "`GET /admin/v1/system/e` has no `x-campbooks-admin`",
            "`GET /admin/v1/system/e` carries `x-campbooks`",
            "`POST /admin/v1/system/f` does not mutate, so it must be GET",
            "`system.version` is also an operation of the catalogue",
            "`GET /admin/v1/jobs/h` must live under `/admin/v1/system/`",
            "operationId `system.admin_overview` is used by both",
            "`GET /admin/v1/system/i` is not served by any module's admin router",
            "`GET /api/v1/system/j` must live under `/admin/v1/system/`",
        ] {
            assert!(v.iter().any(|m| m.contains(expected)), "{expected}: {v:#?}");
        }
    }

    fn source(path: &str, text: &str) -> (String, String) {
        (path.to_owned(), text.to_owned())
    }

    #[test]
    fn only_the_admin_module_and_the_cli_name_the_admin_schema() {
        let ok = [
            source(
                "crates/modules/admin/src/lib.rs",
                r#"sqlx::query!("SELECT permissions FROM admin.profiles WHERE account_id = $1")"#,
            ),
            source(
                "crates/server/src/admin_cli.rs",
                r#"let _ = "DELETE FROM admin.profiles";"#,
            ),
            source(
                "crates/modules/workspaces/src/model.rs",
                "// admin.profiles is not ours\nlet role = \"admin\"; let x = state.admin.profiles; \
                 let q = \"UPDATE workspaces.members SET role = 'admin.x' WHERE c = '\\\"'\";\n\
                 fn f<'a>(c: char) -> bool { c == '\"' || c == '\\'' }",
            ),
        ];
        assert!(
            check_admin_sources(&ok).is_empty(),
            "{:#?}",
            check_admin_sources(&ok)
        );

        let bad = [
            source(
                "crates/server/src/bff/handlers.rs",
                r#"sqlx::query("UPDATE admin.profiles SET permissions = '{super}'")"#,
            ),
            source(
                "crates/modules/identity/src/x.rs",
                "let q = r#\"SELECT 1 FROM \"admin\" . actions\"#;",
            ),
            source(
                "crates/modules/system/src/x.rs",
                r#"let q = "DROP SCHEMA admin CASCADE";"#,
            ),
            source(
                "crates/modules/workspaces/src/x.rs",
                r#"let q = "CREATE POLICY p ON workspaces.users USING (current_setting('campbooks.admin_reads', true) = 'on')";"#,
            ),
            source(
                "crates/kernel/src/db.rs",
                r#"pub const ADMIN_READS_SETTING: &str = "campbooks.admin_reads";"#,
            ),
        ];
        let v = check_admin_sources(&bad);
        assert_eq!(v.len(), 4, "{v:#?}");
        assert!(v[0].contains("crates/server/src/bff/handlers.rs: names schema `admin`"));
        assert!(v[1].contains("crates/modules/identity/src/x.rs: names schema `admin`"));
        assert!(v[2].contains("crates/modules/system/src/x.rs: names schema `admin`"));
        assert!(v[3].contains("crates/modules/workspaces/src/x.rs: names `campbooks.admin_reads`"));
    }

    #[test]
    fn string_literals_are_found_in_every_form() {
        let text = "let a = \"x\\\"y\"; // \"comment\"\n/* \"block\" */ let b = r#\"raw \"q\" \"#; \
                    let c = br\"bytes\"; let d = b\"plain\"; let e = 'z'; let f: &'static str = \"s\"; \
                    let r#type = 1;";
        assert_eq!(
            string_literals(text),
            ["x\"y", "raw \"q\" ", "bytes", "plain", "s"]
        );
    }

    #[test]
    fn parses_architecture_file() {
        let file: ArchitectureFile = toml::from_str("# nothing\n").unwrap();
        assert!(file.allow.is_empty());
        let file: ArchitectureFile =
            toml::from_str("[[allow]]\nfrom = \"mail\"\nto = \"calendar\"\nreason = \"x\"\n")
                .unwrap();
        assert_eq!(
            file.allow,
            vec![Allow {
                from: "mail".into(),
                to: "calendar".into(),
                reason: "x".into()
            }]
        );
    }
}
