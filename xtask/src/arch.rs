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
                 (crates/kernel, crates/ai, crates/server, crates/modules/<name>, xtask)",
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

static MIGRATION_NAME: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(\d{14})_([a-z0-9_]+?)__([a-z0-9_]+)\.sql$")
        .unwrap_or_else(|e| unreachable!("static regex compiles: {e}"))
});

/// Migration files are `<14-digit timestamp>_<owner>__<description>.sql`,
/// where the owner is an existing module or the kernel, and have unique
/// timestamps. No module may take the kernel's name.
pub fn check_migrations(file_names: &[String], modules: &BTreeSet<String>) -> Vec<String> {
    let mut violations = Vec::new();
    let mut versions: BTreeMap<&str, &str> = BTreeMap::new();
    if modules.contains(KERNEL_SCHEMA) {
        violations.push(format!(
            "crates/modules/{KERNEL_SCHEMA}: `{KERNEL_SCHEMA}` is reserved for the kernel's schema"
        ));
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
        if module != KERNEL_SCHEMA && !modules.contains(module) {
            let known: Vec<_> = modules.iter().map(String::as_str).collect();
            violations.push(format!(
                "migrations/{name}: `{module}` is not a module or `{KERNEL_SCHEMA}` (known: {})",
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

static QUALIFIED_NAME: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b([a-z_][a-z0-9_]*)\s*\.\s*[a-z_]")
        .unwrap_or_else(|e| unreachable!("static regex compiles: {e}"))
});

/// Each migration touches only its owner's schema: it creates, alters or
/// drops no other schema, never qualifies a name with another module's (or
/// the kernel's) schema, and never changes `search_path`. `files` are
/// `(file name, SQL)`; files with a malformed name are reported by
/// [`check_migrations`] and skipped here.
pub fn check_migration_schemas(
    files: &[(String, String)],
    modules: &BTreeSet<String>,
) -> Vec<String> {
    let mut violations = Vec::new();
    for (name, sql) in files {
        let Some(owner) = MIGRATION_NAME
            .captures(name)
            .and_then(|caps| caps.get(2))
            .map(|m| m.as_str())
        else {
            continue;
        };
        let sql = SQL_NOISE
            .replace_all(sql, " ")
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
            let known = schema == KERNEL_SCHEMA || modules.contains(schema);
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
        if sql.contains("search_path") {
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

/// Every operation of the generated OpenAPI document states its security
/// explicitly: either public (`security(())`, i.e. `[{}]`), or the `bearer`
/// scheme with exactly the scope from the catalogue that its `Scoped<S>`
/// extractor requires. Nothing is left to a document-wide default.
pub fn check_operation_security(
    spec: &serde_json::Value,
    scheme: &str,
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
            match operation
                .get("security")
                .and_then(serde_json::Value::as_array)
            {
                None => violations.push(format!(
                    "OpenAPI operation `{name}` declares no `security`: require a scope with \
                     `security((\"{scheme}\" = [\"<module>:<action>\"]))` and the \
                     `Scoped<S>` extractor, or mark it public with `security(())`"
                )),
                Some(requirements) => {
                    violations.extend(check_requirements(&name, requirements, scheme, catalogue));
                }
            }
        }
    }
    violations
}

fn check_requirements(
    operation: &str,
    requirements: &[serde_json::Value],
    scheme: &str,
    catalogue: &BTreeSet<String>,
) -> Vec<String> {
    let mut violations = Vec::new();
    for requirement in requirements {
        let Some(requirement) = requirement.as_object() else {
            violations.push(format!(
                "OpenAPI operation `{operation}`: malformed security requirement"
            ));
            continue;
        };
        for (name, scopes) in requirement {
            if name != scheme {
                violations.push(format!(
                    "OpenAPI operation `{operation}` uses security scheme `{name}`; only \
                     `{scheme}` exists"
                ));
                continue;
            }
            let scopes: Vec<&str> = scopes
                .as_array()
                .map(|list| list.iter().filter_map(serde_json::Value::as_str).collect())
                .unwrap_or_default();
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
            Some(_) if undo == id => {
                violations.push(format!("operation `{id}` names itself as its undo"));
            }
            Some(_) => {}
        }
    }
    violations
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
        if !REQUIRED_META.contains(&key.as_str()) && key != "undo" {
            violations.push(format!(
                "OpenAPI operation `{name}`: unknown key `x-campbooks.{key}`"
            ));
        }
    }
    for key in ["mutates", "automation", "assistant"] {
        if meta.get(key).is_some_and(|v| !v.is_boolean()) {
            violations.push(format!(
                "OpenAPI operation `{name}`: `x-campbooks.{key}` must be a boolean"
            ));
        }
    }
    if meta
        .get("risk")
        .is_some_and(|risk| !matches!(risk.as_str(), Some("low" | "high")))
    {
        violations.push(format!(
            "OpenAPI operation `{name}`: `x-campbooks.risk` must be \"low\" or \"high\""
        ));
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
    if let Some(mutates) = meta.get("mutates").and_then(serde_json::Value::as_bool) {
        let writes = matches!(method, "POST" | "PUT" | "PATCH" | "DELETE");
        let reads = matches!(method, "GET" | "HEAD");
        if mutates && !writes {
            violations.push(format!(
                "OpenAPI operation `{name}` mutates, so it must be POST, PUT, PATCH or DELETE"
            ));
        }
        if !mutates && !reads {
            violations.push(format!(
                "OpenAPI operation `{name}` does not mutate, so it must be GET or HEAD"
            ));
        }
    }
    meta.get("mutates")
        .and_then(serde_json::Value::as_bool)
        .map(|mutates| Meta { mutates, undo })
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
        assert_eq!(Kind::from_path("crates/utils"), Kind::Unknown);
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
        assert!(v[0].contains("`billing` is not a module or `kernel`"));
        assert!(v.iter().any(|m| m.contains("already used")));
    }

    #[test]
    fn kernel_owns_its_migrations_and_its_name() {
        let modules: BTreeSet<String> = ["system"].map(String::from).into();
        let names = ["20261012100000_kernel__events_and_jobs.sql".to_owned()];
        assert!(check_migrations(&names, &modules).is_empty());

        let clash: BTreeSet<String> = ["kernel"].map(String::from).into();
        let v = check_migrations(&[], &clash);
        assert_eq!(v.len(), 1, "{v:?}");
        assert!(v[0].contains("reserved"));
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

    #[test]
    fn explicit_security_passes() {
        let spec = serde_json::json!({ "paths": {
            "/api/v1/system/health": { "get": { "security": [{}] } },
            "/api/v1/mail/threads": {
                "get": { "security": [{ "bearer": ["mail:read"] }] },
                "parameters": [],
            },
        }});
        assert!(check_operation_security(&spec, "bearer", &catalogue()).is_empty());
        let public_by_empty_list =
            serde_json::json!({ "paths": { "/x": { "get": { "security": [] } } } });
        assert!(check_operation_security(&public_by_empty_list, "bearer", &catalogue()).is_empty());
    }

    #[test]
    fn operation_without_security_fails() {
        let spec = serde_json::json!({ "paths": {
            "/api/v1/mail/threads": { "get": {}, "post": { "security": [{}] } },
        }});
        let v = check_operation_security(&spec, "bearer", &catalogue());
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
        let v = check_operation_security(&spec, "bearer", &catalogue());
        assert_eq!(v.len(), 4, "{v:#?}");
        assert!(v.iter().any(|m| m.contains("but no scope")));
        assert!(
            v.iter()
                .any(|m| m.contains("`mail:write`, which no module declares"))
        );
        assert!(v.iter().any(|m| m.contains("scheme `apiKey`")));
        assert!(v.iter().any(|m| m.contains("malformed")));
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
