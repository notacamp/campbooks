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
    /// Module functions that call `authz` themselves for one resource type,
    /// so a handler calling one asks the central check about it (e.g.
    /// `lock_conversation(` for `conversations.conversation`).
    #[serde(default)]
    pub helpers: Vec<Helper>,
    /// Module functions that build a reference to one resource type for an
    /// `authorize` call (e.g. `member_resource(` for `workspaces.member`):
    /// a handler passing one to the check names that type.
    #[serde(default)]
    pub references: Vec<Helper>,
    /// Operations whose handler does not call `authz` yet, each with the
    /// issue that moves it on.
    #[serde(default)]
    pub pending: Vec<Pending>,
    /// Files (or directory prefixes) whose transactions without a workspace
    /// are deliberate, with the reason.
    #[serde(default)]
    pub pool_access: Vec<PoolAccess>,
    /// Files outside the kernel that may open a transaction in the kernel
    /// scope (`begin_kernel`, a job's `kernel_tx`), with the reason: module
    /// maintenance that works across workspaces through its tables'
    /// maintenance policies (campbooks-cloud#129). Reviewed like an ADR.
    #[serde(default)]
    pub kernel_scope: Vec<PoolAccess>,
    /// Published events nothing consumes on purpose, with the reason.
    #[serde(default)]
    pub unconsumed_events: Vec<Unconsumed>,
}

/// A function that asks the central check about `resource`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Helper {
    /// How a call to it reads (`lock_conversation(`).
    pub call: String,
    /// The resource type it asks about.
    pub resource: String,
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
    let constants = resource_constants(sources);
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
        // Public operations (`security(())`) authenticate what they act on
        // themselves (a code, a verifier, an ID token hint).
        let public = operation["security"].as_array().is_some_and(|reqs| {
            reqs.iter()
                .any(|r| r.as_object().is_some_and(serde_json::Map::is_empty))
        });
        if !matches!(tenancy, "workspace" | "account") || public {
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
        let declaring = sources.iter().find(|(path, text)| {
            !is_test_file(path) && (text.contains(&declaration) || declares_by_macro(text, id))
        });
        let Some((path, text)) = declaring else {
            violations.push(format!(
                "operation `{name}` (`{id}`): no source file declares it"
            ));
            continue;
        };
        // The operation's own declaration names its resource: only the code
        // around it counts.
        let stripped = without_declarations(without_tests(text));
        let code = stripped.as_str();
        let helper_for = |resource: &str| {
            file.helpers
                .iter()
                .any(|helper| helper.resource == resource && code.contains(helper.call.as_str()))
        };
        let names_it = code.contains(&format!("\"{resource}\""))
            || file
                .references
                .iter()
                .any(|r| r.resource == resource && code.contains(r.call.as_str()))
            || constants
                .iter()
                .filter(|(_, value)| value.as_str() == resource)
                .any(|(constant, _)| mentions(code, constant));
        let asks = (AUTHORIZE_CALL.is_match(code) && names_it) || helper_for(resource);
        if !asks {
            violations.push(format!(
                "{path}: the handler of `{id}` never asks the central check about \
                 `{resource}`: call `authz::require`/`require_type`/`require_account` (or an \
                 `authorize` helper) naming it (ADR 0017)"
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

/// `CONST: &str = "<module>.<record>"` declarations, by name.
fn resource_constants(sources: &[(String, String)]) -> Vec<(String, String)> {
    static CONSTANT: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"const ([A-Z][A-Z0-9_]*): &str = "([a-z_]+\.[a-z_]+)""#)
            .unwrap_or_else(|e| unreachable!("static regex compiles: {e}"))
    });
    sources
        .iter()
        .flat_map(|(_, text)| {
            CONSTANT
                .captures_iter(text)
                .map(|caps| (caps[1].to_owned(), caps[2].to_owned()))
                .collect::<Vec<_>>()
        })
        .collect()
}

/// Whether `code` names the identifier `name` (not as part of another).
fn mentions(code: &str, name: &str) -> bool {
    code.match_indices(name).any(|(at, _)| {
        let before = code[..at].chars().next_back();
        let after = code[at + name.len()..].chars().next();
        let ident = |c: Option<char>| c.is_some_and(|c| c.is_alphanumeric() || c == '_');
        !ident(before) && !ident(after)
    })
}

/// A call to an authorize function: `authorize(`, `authorize_type(`, …
static AUTHORIZE_CALL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"\b(authz::(require|require_type|require_account|decide)|authorize(_type|_account)?)\s*\(",
    )
    .unwrap_or_else(|e| unreachable!("static regex compiles: {e}"))
});

/// `code` without its `#[utoipa::path(…)]` attributes and comments, which
/// name the declared resource whether or not the handler asks about it.
fn without_declarations(code: &str) -> String {
    const ATTRIBUTE: &str = "#[utoipa::path(";
    let mut out = String::with_capacity(code.len());
    let mut rest = code;
    while let Some(at) = rest.find(ATTRIBUTE) {
        out.push_str(&rest[..at]);
        let mut depth = 0usize;
        let mut end = rest.len();
        let (mut quoted, mut escaped) = (false, false);
        for (i, c) in rest[at + 1..].char_indices() {
            if quoted {
                match c {
                    _ if escaped => escaped = false,
                    '\\' => escaped = true,
                    '"' => quoted = false,
                    _ => {}
                }
                continue;
            }
            match c {
                '"' => quoted = true,
                '(' | '[' => depth += 1,
                ')' | ']' => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        end = at + 1 + i + 1;
                        break;
                    }
                }
                _ => {}
            }
        }
        rest = &rest[end..];
    }
    out.push_str(rest);
    out.lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The BFF documents its proxies with a macro taking the id as a literal.
fn declares_by_macro(text: &str, id: &str) -> bool {
    text.contains(&format!("\"{id}\",\n"))
}

/// The code before the file's test module (by convention its last item).
fn without_tests(text: &str) -> &str {
    text.find("#[cfg(test)]\nmod tests {")
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
        Regex::new(
            r"(?is)create\s+table\s+(?:if\s+not\s+exists\s+)?([a-z_]+\.[a-z_]+)\s*\((.*?)\n\);",
        )
        .unwrap_or_else(|e| unreachable!("static regex compiles: {e}"))
    });
    static ADDED: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?is)alter\s+table\s+([a-z_]+\.[a-z_]+)\s+add\s+column\s+workspace_id\b")
            .unwrap_or_else(|e| unreachable!("static regex compiles: {e}"))
    });
    static COLUMN: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^\s*workspace_id\s")
            .unwrap_or_else(|e| unreachable!("static regex compiles: {e}"))
    });
    let mut tables = BTreeSet::new();
    for (_, sql) in migrations {
        for caps in CREATE.captures_iter(sql) {
            if COLUMN.is_match(&caps[2]) {
                tables.insert(caps[1].to_lowercase());
            }
        }
        for caps in ADDED.captures_iter(sql) {
            tables.insert(caps[1].to_lowercase());
        }
    }
    tables
}

/// Code reaches workspace tables only inside a workspace transaction
/// (`begin_for`, a job's `ctx.tx()`) or the kernel scope: no `.begin()`,
/// `.acquire()` or `Acquire::begin` on the pool (a transaction or connection
/// without the workspace set), and no query on a workspace table run
/// straight on the pool, outside the `pool_access` entries. Applies to the
/// modules, the server and the kernel; a query whose SQL is a `const` of the
/// same file counts as that SQL.
pub fn check_workspace_queries(
    sources: &[(String, String)],
    tables: &BTreeSet<String>,
    file: &AccessFile,
) -> Vec<String> {
    static BEGIN: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"\.(begin|acquire)\(\)|\bAcquire::(begin|acquire)\(")
            .unwrap_or_else(|e| unreachable!("static regex compiles: {e}"))
    });
    static CONST: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)\bconst\s+([A-Z][A-Z0-9_]*)\s*:\s*&(?:'static\s+)?str\s*=\s*")
            .unwrap_or_else(|e| unreachable!("static regex compiles: {e}"))
    });
    static IDENT: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"\b[A-Z][A-Z0-9_]*\b")
            .unwrap_or_else(|e| unreachable!("static regex compiles: {e}"))
    });
    static ON_POOL: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"\.(fetch_one|fetch_optional|fetch_all|fetch|execute)\(\s*(&\s*)?(state\.db|self\.db|ctx\.pool\(\)|self\.state\.db|db|pool)\s*\)",
        )
        .unwrap_or_else(|e| unreachable!("static regex compiles: {e}"))
    });
    let allowed = |path: &str| {
        file.pool_access.iter().any(|entry| {
            path == entry.path || (entry.path.ends_with('/') && path.starts_with(&entry.path))
        })
    };
    let mut violations: Vec<String> = file
        .pool_access
        .iter()
        .filter(|entry| entry.reason.trim().is_empty())
        .map(|entry| {
            format!(
                "architecture.toml: pool access `{}` needs a reason",
                entry.path
            )
        })
        .collect();
    let checked = |path: &str| {
        ["crates/modules/", "crates/server/", "crates/kernel/"]
            .iter()
            .any(|prefix| path.starts_with(prefix))
    };
    for (path, text) in sources {
        if !checked(path) || is_test_file(path) || allowed(path) {
            continue;
        }
        let code = without_tests(text);
        if let Some(found) = BEGIN.find(code) {
            violations.push(format!(
                "{path}: `{}` opens a transaction or connection with no workspace set; use \
                 `begin_for(workspace)` or the job's `ctx.tx()`, or list the file under \
                 `[[access.pool_access]]` with the reason",
                found.as_str()
            ));
            continue;
        }
        // SQL kept in a `const` of the file, by name.
        let consts: BTreeMap<&str, Vec<String>> = CONST
            .captures_iter(code)
            .filter_map(|caps| {
                let name = caps.get(1)?.as_str();
                let rest = &code[caps.get(0)?.end()..];
                let end = rest.find(";\n").unwrap_or(rest.len());
                Some((name, string_literals(&rest[..end])))
            })
            .collect();
        for found in ON_POOL.find_iter(code) {
            let start = code[..found.start()]
                .rfind("sqlx::query")
                .unwrap_or(found.start());
            let call = &code[start..found.start()];
            let mut sql = string_literals(call);
            for ident in IDENT.find_iter(call) {
                if let Some(literals) = consts.get(ident.as_str()) {
                    sql.extend(literals.iter().cloned());
                }
            }
            let names: Vec<String> = sql
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

/// The setting that opens the kernel scope (`kernel::db::KERNEL_SCOPE_SETTING`).
pub const KERNEL_SCOPE_SETTING: &str = "campbooks.kernel_scope";

/// Only the kernel names the kernel scope's setting, and only the kernel and
/// the reviewed `kernel_scope` entries open a transaction in it
/// (`begin_kernel(`, `.kernel_tx(`). Entries need a reason and must name a
/// file that calls one (no stale exceptions). Tests may call them.
pub fn check_kernel_scope(sources: &[(String, String)], file: &AccessFile) -> Vec<String> {
    static CALL: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"\bbegin_kernel\s*\(|\.kernel_tx\s*\(")
            .unwrap_or_else(|e| unreachable!("static regex compiles: {e}"))
    });
    let mut violations: Vec<String> = file
        .kernel_scope
        .iter()
        .filter(|entry| entry.reason.trim().is_empty())
        .map(|entry| {
            format!(
                "architecture.toml: kernel scope `{}` needs a reason",
                entry.path
            )
        })
        .collect();
    let mut used = BTreeSet::new();
    for (path, text) in sources {
        if path.starts_with("crates/kernel/") {
            continue;
        }
        if text.to_lowercase().contains(KERNEL_SCOPE_SETTING) {
            violations.push(format!(
                "{path}: names `{KERNEL_SCOPE_SETTING}`; only the kernel may (use \
                 `kernel::db::begin_kernel`)"
            ));
        }
        if is_test_file(path) {
            continue;
        }
        if CALL.is_match(&without_comments(without_tests(text))) {
            if file.kernel_scope.iter().any(|entry| entry.path == *path) {
                used.insert(path.as_str());
            } else {
                violations.push(format!(
                    "{path}: opens the kernel scope (cross-workspace access); work in one \
                     workspace (`begin_for`, the job's `ctx.tx()`), or list the file under \
                     `[[access.kernel_scope]]` with the reason (reviewed like an ADR)"
                ));
            }
        }
    }
    for entry in &file.kernel_scope {
        if !used.contains(entry.path.as_str()) {
            violations.push(format!(
                "architecture.toml: kernel scope `{}` names no file that opens it",
                entry.path
            ));
        }
    }
    violations
}

/// `code` without its line comments.
fn without_comments(code: &str) -> String {
    code.lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
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
            (
                "mail.thread".to_owned(),
                resource("workspace", &["read", "write"]),
            ),
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
        let src = |text: &str| {
            vec![(
                "crates/modules/mail/src/list.rs".to_owned(),
                text.to_owned(),
            )]
        };
        assert_eq!(
            check_authorize_calls(&s, &c, &src(declared), &file).len(),
            1
        );
        // Naming the action in `x-campbooks` is not asking.
        let named = format!("{declared}// .access(\"mail.thread\", authz::Action::Read)\n");
        assert_eq!(check_authorize_calls(&s, &c, &src(&named), &file).len(), 1);
        let asking = format!("{declared}fn h() {{ authz::require_type(\"mail.thread\"); }}\n");
        // Asking about another resource is not asking about this one.
        let other = format!("{declared}fn h() {{ authz::require_type(\"mail.box\"); }}\n");
        assert_eq!(check_authorize_calls(&s, &c, &src(&other), &file).len(), 1);
        let constant = format!(
            "{declared}const THREAD: &str = \"mail.thread\";\nfn h() {{ authz::require_type(THREAD); }}\n"
        );
        assert!(check_authorize_calls(&s, &c, &src(&constant), &file).is_empty());
        assert!(check_authorize_calls(&s, &c, &src(&asking), &file).is_empty());
        // The declaration's own mention of the resource (in its
        // `#[utoipa::path]`, strings with parentheses included) does not make
        // a call about another resource count.
        let attribute = "#[utoipa::path(\n    operation_id = \"mail.list\",\n    \
            extensions((\"x-campbooks\" = json!(Operation::query().access(\"mail.thread\", \
            Action::Read)))),\n    responses((status = OK, description = \"Fine (or not\"))\n)]\n";
        let elsewhere = format!("{attribute}fn h() {{ authz::require_type(\"mail.box\"); }}\n");
        assert_eq!(
            check_authorize_calls(&s, &c, &src(&elsewhere), &file).len(),
            1
        );
        let right = format!("{attribute}fn h() {{ authz::require_type(\"mail.thread\"); }}\n");
        let elsewhere_head = attribute;
        assert!(check_authorize_calls(&s, &c, &src(&right), &file).is_empty());
        // A call inside the test module does not count.
        let in_tests = format!("{declared}#[cfg(test)]\nmod tests {{ authz::x(); }}\n");
        assert_eq!(
            check_authorize_calls(&s, &c, &src(&in_tests), &file).len(),
            1
        );
        let helper = format!("{declared}fn h() {{ lock_thread(&state); }}\n");
        let with_helper = AccessFile {
            helpers: vec![Helper {
                call: "lock_thread(".to_owned(),
                resource: "mail.thread".to_owned(),
            }],
            ..AccessFile::default()
        };
        assert!(check_authorize_calls(&s, &c, &src(&helper), &with_helper).is_empty());
        // A function building the reference counts as naming it, only when
        // the handler also asks the check.
        let with_reference = AccessFile {
            references: vec![Helper {
                call: "thread_ref(".to_owned(),
                resource: "mail.thread".to_owned(),
            }],
            ..AccessFile::default()
        };
        let referenced = format!("{elsewhere_head}fn h() {{ authorize(&s, thread_ref(id)); }}\n");
        assert!(check_authorize_calls(&s, &c, &src(&referenced), &with_reference).is_empty());
        let unasked = format!("{elsewhere_head}fn h() {{ let r = thread_ref(id); }}\n");
        assert_eq!(
            check_authorize_calls(&s, &c, &src(&unasked), &with_reference).len(),
            1
        );
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
        assert_eq!(
            check_authorize_calls(&s, &c, &src(&asking), &stale).len(),
            1
        );
    }

    #[test]
    fn workspace_tables_come_from_the_migrations() {
        let sql = "CREATE TABLE mail.threads (\n    id uuid,\n    workspace_id uuid NOT NULL\n);\n\
                   CREATE TABLE mail.budgets (\n    provider text,\n    target_workspace_id uuid\n);\n";
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
                &[(
                    "crates/modules/mail/src/a.rs".to_owned(),
                    on_pool.to_owned()
                )],
                &tables,
                &excused
            )
            .is_empty()
        );
    }

    #[test]
    fn connections_constants_and_the_server_and_kernel_are_checked_too() {
        let tables = BTreeSet::from(["mail.threads".to_owned()]);
        let file = AccessFile::default();
        let check = |path: &str, text: &str| {
            check_workspace_queries(&[(path.to_owned(), text.to_owned())], &tables, &file)
        };
        let module = "crates/modules/mail/src/a.rs";
        assert_eq!(check(module, "let c = state.db.acquire().await?;").len(), 1);
        assert_eq!(
            check(module, "let t = Acquire::begin(&state.db).await?;").len(),
            1
        );
        let constant = "const SQL: &str = \"SELECT id FROM mail.threads\";\n\
                        fn f() { sqlx::query(SQL).fetch_all(&state.db).await; }";
        let found = check(module, constant);
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].contains("mail.threads"), "{found:?}");
        let on_pool = r#"sqlx::query!("SELECT id FROM mail.threads").fetch_all(&state.db).await"#;
        assert_eq!(check("crates/server/src/x.rs", on_pool).len(), 1);
        assert_eq!(
            check("crates/kernel/src/x.rs", "let tx = pool.begin().await?;").len(),
            1
        );
        assert!(check("xtask/src/x.rs", on_pool).is_empty());
        assert!(check("crates/server/src/x_tests.rs", on_pool).is_empty());
    }

    #[test]
    fn only_the_kernel_and_reviewed_files_open_the_kernel_scope() {
        let entry = |path: &str, reason: &str| PoolAccess {
            path: path.to_owned(),
            reason: reason.to_owned(),
        };
        let module = "crates/modules/mail/src/dispatch.rs";
        let calls = "let tx = db::begin_kernel(&pool).await?;";
        let source = |path: &str, text: &str| vec![(path.to_owned(), text.to_owned())];
        let none = AccessFile::default();
        assert_eq!(check_kernel_scope(&source(module, calls), &none).len(), 1);
        assert_eq!(
            check_kernel_scope(&source(module, "let tx = ctx.kernel_tx().await?;"), &none).len(),
            1
        );
        assert!(
            check_kernel_scope(&source("crates/kernel/src/jobs/worker.rs", calls), &none)
                .is_empty()
        );
        assert!(
            check_kernel_scope(&source("crates/modules/mail/src/tests.rs", calls), &none)
                .is_empty()
        );
        let named = r#"sqlx::query("SELECT set_config('campbooks.kernel_scope', 'on', true)")"#;
        let found = check_kernel_scope(&source("crates/server/src/x.rs", named), &none);
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].contains("names `campbooks.kernel_scope`"));
        let found = check_kernel_scope(&source("crates/modules/mail/src/x_tests.rs", named), &none);
        assert_eq!(found.len(), 1, "tests may not name it either: {found:?}");

        let allowed = AccessFile {
            kernel_scope: vec![entry(module, "the scheduler")],
            ..AccessFile::default()
        };
        assert!(check_kernel_scope(&source(module, calls), &allowed).is_empty());
        let stale = check_kernel_scope(&source(module, "fn nothing() {}"), &allowed);
        assert_eq!(stale.len(), 1, "{stale:?}");
        assert!(stale[0].contains("names no file"));
        let unexplained = AccessFile {
            kernel_scope: vec![entry(module, " ")],
            ..AccessFile::default()
        };
        assert_eq!(
            check_kernel_scope(&source(module, calls), &unexplained).len(),
            1
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
