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

static MIGRATION_NAME: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(\d{14})_([a-z0-9_]+?)__([a-z0-9_]+)\.sql$")
        .unwrap_or_else(|e| unreachable!("static regex compiles: {e}"))
});

/// Migration files are `<14-digit timestamp>_<module>__<description>.sql`,
/// name an existing module, and have unique timestamps.
pub fn check_migrations(file_names: &[String], modules: &BTreeSet<String>) -> Vec<String> {
    let mut violations = Vec::new();
    let mut versions: BTreeMap<&str, &str> = BTreeMap::new();

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
        if !modules.contains(module) {
            let known: Vec<_> = modules.iter().map(String::as_str).collect();
            violations.push(format!(
                "migrations/{name}: `{module}` is not a module (known: {})",
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
const OPERATION_METHODS: [&str; 8] = [
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
        assert!(v[0].contains("`billing` is not a module"));
        assert!(v.iter().any(|m| m.contains("already used")));
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
