//! Access rules (ADR 0017, `openspec/changes/access-guards`): every
//! operation declares the resource type and action it takes, catalogued;
//! every handler acting in a workspace or for an account asks the central
//! check; module code queries workspace tables only inside a workspace
//! transaction; every published event has a consumer or a stated reason.
//!
//! Pure functions over plain data, like `arch.rs`; `main.rs` gathers it.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::LazyLock;

use regex::Regex;
use serde::Deserialize;

use crate::arch::{OPERATION_METHODS, string_literals};

/// A catalogued resource type, as the rules need it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resource {
    /// `workspace`, `account`, `instance` or `public`.
    pub tenancy: String,
    /// Every action anyone may take on it.
    pub actions: BTreeSet<String>,
}

/// `[access]` of `architecture.toml`.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccessFile {
    /// Module functions that call `authz` themselves, so a handler calling
    /// one asks the central check (e.g. `lock_conversation(`).
    #[serde(default)]
    pub helpers: Vec<String>,
    /// Operations whose handler does not call `authz` yet, each with the
    /// issue that moves it on.
    #[serde(default)]
    pub pending: Vec<Pending>,
    /// Files (or directory prefixes) whose transactions without a workspace
    /// are deliberate, with the reason.
    #[serde(default)]
    pub pool_access: Vec<PoolAccess>,
    /// Published events nothing consumes on purpose, with the reason.
    #[serde(default)]
    pub unconsumed_events: Vec<Unconsumed>,
}

/// An operation not on the central check yet.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pending {
    /// Its `operationId`.
    pub operation: String,
    /// The tracking issue.
    pub issue: String,
}

/// A deliberate transaction without a workspace.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PoolAccess {
    /// A file, or a directory prefix ending in `/`.
    pub path: String,
    /// Why.
    pub reason: String,
}

/// An event nothing consumes.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Unconsumed {
    /// `DomainEvent::NAME`.
    pub name: String,
    /// Why (e.g. it exists for the change feed, or for automation).
    pub reason: String,
}

/// Every operation of `spec` declares `x-campbooks.access` naming a
/// catalogued resource type and an action declared for it.
pub fn check_declarations(
    spec: &serde_json::Value,
    catalogue: &BTreeMap<String, Resource>,
) -> Vec<String> {
    let mut violations = Vec::new();
    for (name, operation) in operations(spec) {
        let Some(access) = operation.pointer("/x-campbooks/access") else {
            violations.push(format!(
                "operation `{name}` declares no access: add `.access(\"<module>.<record>\", \
                 Action::…)` to its `x-campbooks` (ADR 0017)"
            ));
            continue;
        };
        let resource = access["resource"].as_str().unwrap_or_default();
        let action = access["action"].as_str().unwrap_or_default();
        match catalogue.get(resource) {
            None => violations.push(format!(
                "operation `{name}` acts on `{resource}`, which no module declares in its \
                 `resources`"
            )),
            Some(declared) if !declared.actions.contains(action) => {
                violations.push(format!(
                    "operation `{name}` takes `{action}` on `{resource}`, an action nobody \
                     is granted on it"
                ));
            }
            Some(_) => {}
        }
    }
    violations
}

/// Every operation acting in a workspace or for an account has a handler
/// that asks the central check: the source file declaring its
/// `operation_id` calls `authz::`, an `authorize…(` function, or one of
/// `helpers`; unless it is `pending` (with an issue). `sources` are
/// `(path, text)`.
pub fn check_authorize_calls(
    spec: &serde_json::Value,
    catalogue: &BTreeMap<String, Resource>,
    sources: &[(String, String)],
    file: &AccessFile,
) -> Vec<String> {
    let mut violations = Vec::new();
    let pending: BTreeMap<&str, &Pending> = file
        .pending
        .iter()
        .map(|p| (p.operation.as_str(), p))
        .collect();
    let mut seen_pending = BTreeSet::new();
    for (name, operation) in operations(spec) {
        let Some(id) = operation["operationId"].as_str() else {
            continue;
        };
        let resource = operation
            .pointer("/x-campbooks/access/resource")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        let tenancy = catalogue
            .get(resource)
            .map_or("", |declared| declared.tenancy.as_str());
        if !matches!(tenancy, "workspace" | "account") {
            continue;
        }
        if let Some(entry) = pending.get(id) {
            seen_pending.insert(id.to_owned());
            if !entry.issue.starts_with("https://") {
                violations.push(format!(
                    "architecture.toml: pending operation `{id}` needs its tracking issue URL"
                ));
            }
            continue;
        }
        let declaration = format!("operation_id = \"{id}\"");
        let declaring = sources
            .iter()
            .find(|(_, text)| text.contains(&declaration) || declares_by_macro(text, id));
        let Some((path, text)) = declaring else {
            violations.push(format!(
                "operation `{name}` (`{id}`): no source file declares it"
            ));
            continue;
        };
        let code = without_tests(text);
        let asks = code.contains("authz::")
            || AUTHORIZE_CALL.is_match(code)
            || file.helpers.iter().any(|helper| code.contains(helper.as_str()));
        if !asks {
            violations.push(format!(
                "{path}: the handler of `{id}` never asks the central check: call \
                 `authz::require`/`require_type`/`require_account` (or an `authorize` \
                 helper) for `{resource}` (ADR 0017)"
            ));
        }
    }
    for entry in &file.pending {
        if !seen_pending.contains(&entry.operation) {
            violations.push(format!(
                "architecture.toml: pending operation `{}` is not a workspace or account \
                 operation of the catalogue; remove the entry",
                entry.operation
            ));
        }
    }
    violations
}

/// A call to an authorize function: `authorize(`, `authorize_type(`, …
static AUTHORIZE_CALL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\bauthorize(_type|_account)?\s*\(")
        .unwrap_or_else(|e| unreachable!("static regex compiles: {e}"))
});

/// The BFF documents its proxies with a macro taking the id as a literal.
fn declares_by_macro(text: &str, id: &str) -> bool {
    text.contains(&format!("\"{id}\",\n"))
}

/// The code before the file's test module (by convention its last item).
fn without_tests(text: &str) -> &str {
    text.find("#[cfg(test)]\nmod tests")
        .map_or(text, |at| &text[..at])
}

/// Every `(METHOD path, operation)` of the document.
fn operations(spec: &serde_json::Value) -> Vec<(String, &serde_json::Value)> {
    let mut out = Vec::new();
    if let Some(paths) = spec.get("paths").and_then(serde_json::Value::as_object) {
        for (path, item) in paths {
            for method in OPERATION_METHODS {
                if let Some(operation) = item.get(method) {
                    out.push((format!("{} {path}", method.to_uppercase()), operation));
                }
            }
        }
    }
    out
}

/// The workspace-owned tables (`schema.table`) the migrations create: those
/// with a `workspace_id` column.
pub fn workspace_tables(migrations: &[(String, String)]) -> BTreeSet<String> {
    static CREATE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?is)create\s+table\s+(?:if\s+not\s+exists\s+)?([a-z_]+\.[a-z_]+)\s*\((.*?)\n\);")
            .unwrap_or_else(|e| unreachable!("static regex compiles: {e}"))
    });
    static ADDED: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?is)alter\s+table\s+([a-z_]+\.[a-z_]+)\s+add\s+column\s+workspace_id\b")
            .unwrap_or_else(|e| unreachable!("static regex compiles: {e}"))
    });
    let mut tables = BTreeSet::new();
    for (_, sql) in migrations {
        for caps in CREATE.captures_iter(sql) {
            if caps[2].contains("workspace_id") {
                tables.insert(caps[1].to_lowercase());
            }
        }
        for caps in ADDED.captures_iter(sql) {
            tables.insert(caps[1].to_lowercase());
        }
    }
    tables
}

/// Module code reaches workspace tables only inside a workspace
/// transaction (`begin_for`, a job's `ctx.tx()`): no `.begin()` (a
/// transaction without the workspace set), and no query on a workspace
/// table run straight on the pool, outside the `pool_access` entries.
pub fn check_workspace_queries(
    sources: &[(String, String)],
    tables: &BTreeSet<String>,
    file: &AccessFile,
) -> Vec<String> {
    static BEGIN: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"\.begin\(\)")
            .unwrap_or_else(|e| unreachable!("static regex compiles: {e}"))
    });
    static ON_POOL: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"\.(fetch_one|fetch_optional|fetch_all|fetch|execute)\(\s*(&\s*)?(state\.db|self\.db|ctx\.pool\(\)|self\.state\.db|db|pool)\s*\)",
        )
        .unwrap_or_else(|e| unreachable!("static regex compiles: {e}"))
    });
    let allowed = |path: &str| {
        file.pool_access
            .iter()
            .any(|entry| path == entry.path || (entry.path.ends_with('/') && path.starts_with(&entry.path)))
    };
    let mut violations = Vec::new();
    for (path, text) in sources {
        if !path.starts_with("crates/modules/") || is_test_file(path) || allowed(path) {
            continue;
        }
        let code = without_tests(text);
        if BEGIN.is_match(code) {
            violations.push(format!(
                "{path}: `.begin()` opens a transaction with no workspace set; use \
                 `begin_for(workspace)` or the job's `ctx.tx()`, or list the file under \
                 `[[access.pool_access]]` with the reason"
            ));
            continue;
        }
        for found in ON_POOL.find_iter(code) {
            let start = code[..found.start()].rfind("sqlx::query").unwrap_or(found.start());
            let names: Vec<String> = string_literals(&code[start..found.start()])
                .iter()
                .flat_map(|sql| qualified_names(sql))
                .filter(|name| tables.contains(name))
                .collect();
            if let Some(table) = names.first() {
                violations.push(format!(
                    "{path}: a query on workspace table `{table}` runs on the pool, without \
                     the workspace set; run it in `begin_for(workspace)` or the job's \
                     `ctx.tx()`"
                ));
                break;
            }
        }
    }
    violations
}

fn is_test_file(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or_default();
    name == "tests.rs"
        || name.ends_with("_tests.rs")
        || name.starts_with("testkit")
        || path.contains("/tests/")
}

fn qualified_names(sql: &str) -> Vec<String> {
    static NAME: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"\b([a-z_]+\.[a-z_]+)\b")
            .unwrap_or_else(|e| unreachable!("static regex compiles: {e}"))
    });
    NAME.captures_iter(&sql.to_lowercase())
        .map(|caps| caps[1].to_owned())
        .collect()
}

/// Every published event has a consumer, or an `unconsumed_events` entry
/// with the reason (an event kept for the change feed, for automation to
/// come). Entries must name published, unconsumed events.
pub fn check_consumers(
    published: &BTreeSet<String>,
    consumed: &BTreeSet<String>,
    file: &AccessFile,
) -> Vec<String> {
    let mut violations = Vec::new();
    let excused: BTreeMap<&str, &Unconsumed> = file
        .unconsumed_events
        .iter()
        .map(|entry| (entry.name.as_str(), entry))
        .collect();
    for name in published.difference(consumed) {
        match excused.get(name.as_str()) {
            None => violations.push(format!(
                "event `{name}` is published but nothing consumes it: add a consumer, or an \
                 `[[access.unconsumed_events]]` entry saying why"
            )),
            Some(entry) if entry.reason.trim().is_empty() => violations.push(format!(
                "architecture.toml: unconsumed event `{name}` needs a reason"
            )),
            Some(_) => {}
        }
    }
    for entry in &file.unconsumed_events {
        if !published.contains(&entry.name) || consumed.contains(&entry.name) {
            violations.push(format!(
                "architecture.toml: `{}` is listed as unconsumed but is {}; remove the entry",
                entry.name,
                if published.contains(&entry.name) {
                    "consumed"
                } else {
                    "not published"
                }
            ));
        }
    }
    violations
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn catalogue() -> BTreeMap<String, Resource> {
        let resource = |tenancy: &str, actions: &[&str]| Resource {
            tenancy: tenancy.to_owned(),
            actions: actions.iter().map(|a| (*a).to_owned()).collect(),
        };
        [
            ("mail.thread".to_owned(), resource("workspace", &["read", "write"])),
            ("system.health".to_owned(), resource("public", &["read"])),
        ]
        .into()
    }

    fn spec(access: Option<serde_json::Value>) -> serde_json::Value {
        let mut meta = json!({"mutates": false});
        if let Some(access) = access {
            meta["access"] = access;
        }
        json!({"paths": {"/api/v1/w/{workspace_id}/threads": {"get": {
            "operationId": "mail.list", "x-campbooks": meta
        }}}})
    }

    #[test]
    fn operations_declare_a_catalogued_resource_and_action() {
        let c = catalogue();
        let ok = spec(Some(json!({"resource": "mail.thread", "action": "read"})));
        assert!(check_declarations(&ok, &c).is_empty());
        assert_eq!(check_declarations(&spec(None), &c).len(), 1);
        let unknown = spec(Some(json!({"resource": "mail.box", "action": "read"})));
        assert!(check_declarations(&unknown, &c)[0].contains("no module declares"));
        let action = spec(Some(json!({"resource": "mail.thread", "action": "share"})));
        assert!(check_declarations(&action, &c)[0].contains("nobody"));
    }

    #[test]
    fn workspace_handlers_ask_the_central_check() {
        let c = catalogue();
        let s = spec(Some(json!({"resource": "mail.thread", "action": "read"})));
        let declared = "#[utoipa::path(operation_id = \"mail.list\")]\nfn handler() {}\n";
        let file = AccessFile::default();
        let src = |text: &str| vec![("crates/modules/mail/src/list.rs".to_owned(), text.to_owned())];
        assert_eq!(check_authorize_calls(&s, &c, &src(declared), &file).len(), 1);
        let asking = format!("{declared}fn h() {{ authz::require_type(); }}\n");
        assert!(check_authorize_calls(&s, &c, &src(&asking), &file).is_empty());
        // A call inside the test module does not count.
        let in_tests = format!("{declared}#[cfg(test)]\nmod tests {{ authz::x(); }}\n");
        assert_eq!(check_authorize_calls(&s, &c, &src(&in_tests), &file).len(), 1);
        let helper = format!("{declared}fn h() {{ lock_thread(&state); }}\n");
        let with_helper = AccessFile {
            helpers: vec!["lock_thread(".to_owned()],
            ..AccessFile::default()
        };
        assert!(check_authorize_calls(&s, &c, &src(&helper), &with_helper).is_empty());
        let pending = AccessFile {
            pending: vec![Pending {
                operation: "mail.list".to_owned(),
                issue: "https://example.com/issues/1".to_owned(),
            }],
            ..AccessFile::default()
        };
        assert!(check_authorize_calls(&s, &c, &src(declared), &pending).is_empty());
        let stale = AccessFile {
            pending: vec![Pending {
                operation: "mail.gone".to_owned(),
                issue: "https://example.com/issues/1".to_owned(),
            }],
            ..AccessFile::default()
        };
        assert_eq!(check_authorize_calls(&s, &c, &src(&asking), &stale).len(), 1);
    }

    #[test]
    fn workspace_tables_come_from_the_migrations() {
        let sql = "CREATE TABLE mail.threads (\n    id uuid,\n    workspace_id uuid NOT NULL\n);\n\
                   CREATE TABLE mail.budgets (\n    provider text\n);\n";
        let tables = workspace_tables(&[("1_mail__init.sql".to_owned(), sql.to_owned())]);
        assert_eq!(tables, BTreeSet::from(["mail.threads".to_owned()]));
    }

    #[test]
    fn workspace_tables_are_queried_in_a_workspace_transaction() {
        let tables = BTreeSet::from(["mail.threads".to_owned()]);
        let file = AccessFile::default();
        let check = |text: &str| {
            check_workspace_queries(
                &[("crates/modules/mail/src/a.rs".to_owned(), text.to_owned())],
                &tables,
                &file,
            )
        };
        assert!(check("let tx = state.db.begin_for(ws).await?;").is_empty());
        assert_eq!(check("let tx = state.db.begin().await?;").len(), 1);
        let on_pool = r#"sqlx::query!("SELECT id FROM mail.threads").fetch_all(&state.db).await"#;
        assert_eq!(check(on_pool).len(), 1);
        let instance = r#"sqlx::query!("SELECT 1 FROM mail.budgets").fetch_all(&state.db).await"#;
        assert!(check(instance).is_empty());
        let in_tx = r#"sqlx::query!("SELECT id FROM mail.threads").fetch_all(&mut *tx).await"#;
        assert!(check(in_tx).is_empty());
        let excused = AccessFile {
            pool_access: vec![PoolAccess {
                path: "crates/modules/mail/".to_owned(),
                reason: "instance-wide".to_owned(),
            }],
            ..AccessFile::default()
        };
        assert!(
            check_workspace_queries(
                &[("crates/modules/mail/src/a.rs".to_owned(), on_pool.to_owned())],
                &tables,
                &excused
            )
            .is_empty()
        );
    }

    #[test]
    fn published_events_need_a_consumer_or_a_reason() {
        let published = BTreeSet::from(["a.one".to_owned(), "a.two".to_owned()]);
        let consumed = BTreeSet::from(["a.one".to_owned()]);
        assert_eq!(
            check_consumers(&published, &consumed, &AccessFile::default()).len(),
            1
        );
        let excused = AccessFile {
            unconsumed_events: vec![Unconsumed {
                name: "a.two".to_owned(),
                reason: "the change feed".to_owned(),
            }],
            ..AccessFile::default()
        };
        assert!(check_consumers(&published, &consumed, &excused).is_empty());
        let stale = AccessFile {
            unconsumed_events: vec![Unconsumed {
                name: "a.one".to_owned(),
                reason: "x".to_owned(),
            }],
            ..AccessFile::default()
        };
        assert_eq!(check_consumers(&published, &consumed, &stale).len(), 2);
    }
}
