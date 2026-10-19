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
    /// Files outside the kernel that may open a transaction in the
    /// maintenance scope (`begin_maintenance`, a job's `maintenance_tx`),
    /// with the reason: a scheduler's dispatcher reading its own index
    /// across workspaces through its table's maintenance policies, which no
    /// kernel table admits (campbooks-cloud#196). Reviewed like an ADR.
    #[serde(default)]
    pub maintenance_scope: Vec<PoolAccess>,
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
        let (path, text) = match declaring_file(sources, id) {
            Declaring::One(path, text) => (path, text),
            Declaring::None => {
                violations.push(format!(
                    "operation `{name}` (`{id}`): no source file declares it"
                ));
                continue;
            }
            Declaring::Many(paths) => {
                violations.push(format!(
                    "operation `{name}` (`{id}`) is declared in more than one file ({}): \
                     declare it once, so the check knows which handler to read (ADR 0017)",
                    paths.join(", ")
                ));
                continue;
            }
        };
        // The operation's own declaration names its resource: only the code
        // around it counts, and no comment (line, trailing or block) does.
        let stripped = without_declarations(&without_comments(without_tests(text)));
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

/// Which file declares an operation, as `declaring_file` finds it.
enum Declaring<'a> {
    /// No file does.
    None,
    /// This one: `(path, text)`.
    One(&'a str, &'a str),
    /// Several hold a real `operation_id = "<id>"`: their paths, sorted.
    Many(Vec<&'a str>),
}

/// The non-test file declaring operation `id`, read on its code only: a
/// comment quoting a declaration (the kernel's operations docs show one),
/// line, doc or block, is not one, nor is code under `#[cfg(test)]`. A real
/// `operation_id = "<id>"` outranks a macro's `"<id>",` literal (rate-limit
/// tables and catalogues hold those too); two real ones are ambiguous; among
/// literals the first path wins. The order the files were read in never
/// decides.
fn declaring_file<'a>(sources: &'a [(String, String)], id: &str) -> Declaring<'a> {
    let declaration = format!("operation_id = \"{id}\"");
    let (mut real, mut listed) = (Vec::new(), Vec::new());
    for (path, text) in sources {
        if is_test_file(path) || !(text.contains(&declaration) || declares_by_macro(text, id)) {
            continue;
        }
        let code = without_comments(without_tests(text));
        if code.contains(&declaration) {
            real.push((path.as_str(), text.as_str()));
        } else if declares_by_macro(&code, id) {
            listed.push((path.as_str(), text.as_str()));
        }
    }
    match *real.as_slice() {
        [(path, text)] => Declaring::One(path, text),
        [] => listed
            .into_iter()
            .min_by_key(|&(path, _)| path)
            .map_or(Declaring::None, |(path, text)| Declaring::One(path, text)),
        _ => {
            let mut paths: Vec<&str> = real.iter().map(|&(path, _)| path).collect();
            paths.sort_unstable();
            Declaring::Many(paths)
        }
    }
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

/// `code` (its comments already removed) without its `#[utoipa::path(…)]`
/// attributes, which name the declared resource whether or not the handler
/// asks about it.
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
    out
}

/// The BFF documents its proxies with a macro taking the id as a literal.
fn declares_by_macro(text: &str, id: &str) -> bool {
    text.contains(&format!("\"{id}\",\n"))
}

/// The code before the file's test module (by convention its last item;
/// clippy's `items_after_test_module` keeps it so). The cut is looked for in
/// code only, so the same text inside a comment or a string does not end the
/// file early; only at the file's top level (bracket depth 0), so a test
/// module inside another module or a `macro_rules!` body does not either;
/// and only when nothing but whitespace (or comments) follows the module's
/// closing brace (campbooks-cloud#207). Anything else is checked whole.
pub(crate) fn without_tests(text: &str) -> &str {
    test_module_start(&code_only(text)).map_or(text, |at| &text[..at])
}

/// Where the trailing top-level `#[cfg(test)] mod tests { … }` of `code`
/// (comments and literals blanked) starts.
fn test_module_start(code: &str) -> Option<usize> {
    const ATTRIBUTE: &str = "#[cfg(test)]";
    let bytes = code.as_bytes();
    let mut depth = 0usize;
    for (i, &b) in bytes.iter().enumerate() {
        match b {
            b'{' | b'(' | b'[' => depth += 1,
            b'}' | b')' | b']' => depth = depth.saturating_sub(1),
            b'#' if depth == 0 && code[i..].starts_with(ATTRIBUTE) => {
                let after = i + ATTRIBUTE.len();
                let Some(open) = module_body(&code[after..]).map(|at| after + at) else {
                    continue;
                };
                if let Some(close) = closing(bytes, open)
                    && code[close + 1..].trim().is_empty()
                {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

/// In `rest` (what follows a `#[cfg(test)]`): more attributes, then
/// `mod tests {`; the offset of that `{`.
fn module_body(rest: &str) -> Option<usize> {
    let bytes = rest.as_bytes();
    let mut i = 0;
    let skip_space = |i: &mut usize| {
        while bytes.get(*i).is_some_and(u8::is_ascii_whitespace) {
            *i += 1;
        }
    };
    skip_space(&mut i);
    while rest[i..].starts_with("#[") {
        i = closing(bytes, i + 1)? + 1;
        skip_space(&mut i);
    }
    let rest_here = &rest[i..];
    let words = rest_here.strip_prefix("mod")?;
    if !words.starts_with(|c: char| c.is_ascii_whitespace()) {
        return None;
    }
    let name = words.trim_start();
    let body = name.strip_prefix("tests")?.trim_start();
    body.starts_with('{').then(|| rest.len() - body.len())
}

/// The bracket closing the one opened at `open` (any of `{[(`, nested).
fn closing(bytes: &[u8], open: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (i, &b) in bytes.iter().enumerate().skip(open) {
        match b {
            b'{' | b'(' | b'[' => depth += 1,
            b'}' | b')' | b']' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
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

/// Which reviewed list of `architecture.toml` lets a file outside the kernel
/// use one of the kernel's scope names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Door {
    /// None: only the kernel names it.
    Never,
    /// `[[access.kernel_scope]]`.
    Kernel,
    /// `[[access.maintenance_scope]]`.
    Maintenance,
}

impl Door {
    /// The list's name in `architecture.toml`.
    fn list(self) -> &'static str {
        match self {
            Self::Never => "",
            Self::Kernel => "kernel_scope",
            Self::Maintenance => "maintenance_scope",
        }
    }

    /// The reviewed entries of that list.
    fn entries(self, file: &AccessFile) -> &[PoolAccess] {
        match self {
            Self::Never => &[],
            Self::Kernel => &file.kernel_scope,
            Self::Maintenance => &file.maintenance_scope,
        }
    }
}

/// The settings that open a cross-workspace scope (the kernel's
/// `KERNEL_SCOPE_SETTING`, `MAINTENANCE_SCOPE_SETTING`), each with its last
/// word: outside the kernel, the word on its own (in code or a string, any
/// case) usually means the setting's name is being built at runtime.
/// Each with the kernel function that opens it. The admin reads' setting
/// has no reviewed list: only `begin_admin` (with an `AdminGrant`) sets it;
/// nor has the idempotency caller's (campbooks-cloud#210): only the kernel's
/// idempotency layer sets it.
const SCOPE_SETTINGS: [(&str, &str, Door, &str); 4] = [
    (
        "campbooks.kernel_scope",
        "kernel_scope",
        Door::Kernel,
        "begin_kernel",
    ),
    (
        "campbooks.maintenance_scope",
        "maintenance_scope",
        Door::Maintenance,
        "begin_maintenance",
    ),
    (
        "campbooks.admin_reads",
        "admin_reads",
        Door::Never,
        "begin_admin",
    ),
    (
        "campbooks.idempotency_caller",
        "idempotency_caller",
        Door::Never,
        "begin_as_idempotency_caller",
    ),
];

/// The kernel's names that open a cross-workspace scope or name its
/// setting: outside the kernel, each is a violation wherever it appears in
/// code (a call, a `use … as` rename, `JobContext::kernel_tx(ctx)`, a
/// function value), comments aside, unless its [`Door`] lists the file.
const SCOPE_IDENTIFIERS: [(&str, Door); 8] = [
    ("KERNEL_SCOPE_SETTING", Door::Never),
    ("MAINTENANCE_SCOPE_SETTING", Door::Never),
    ("ADMIN_READS_SETTING", Door::Never),
    ("IDEMPOTENCY_CALLER_SETTING", Door::Never),
    ("begin_kernel", Door::Kernel),
    ("kernel_tx", Door::Kernel),
    ("begin_maintenance", Door::Maintenance),
    ("maintenance_tx", Door::Maintenance),
];

/// Only the kernel names a scope's setting (`campbooks.kernel_scope`,
/// `campbooks.maintenance_scope`, in any code, tests included), and outside
/// the kernel no code uses the identifiers `KERNEL_SCOPE_SETTING`,
/// `MAINTENANCE_SCOPE_SETTING` or `ADMIN_READS_SETTING`, nor a scope's
/// openers (`begin_kernel` / `kernel_tx`, `begin_maintenance` /
/// `maintenance_tx`) except in that scope's reviewed entries
/// (`kernel_scope`, `maintenance_scope`). Nor, outside the kernel and those
/// entries, does code or a string name a setting's last word on its own
/// (`kernel_scope`, `maintenance_scope`: a setting name built at runtime,
/// tests included). Test code is exempt from the identifier rule. Entries
/// need a reason and must name a file that uses one of their openers (no
/// stale exceptions).
pub fn check_kernel_scope(sources: &[(String, String)], file: &AccessFile) -> Vec<String> {
    static IDENTIFIER: LazyLock<Regex> = LazyLock::new(|| {
        let names: Vec<&str> = SCOPE_IDENTIFIERS.iter().map(|(name, _)| *name).collect();
        Regex::new(&format!(r"\b({})\b", names.join("|")))
            .unwrap_or_else(|e| unreachable!("static regex compiles: {e}"))
    });
    let doors = [Door::Kernel, Door::Maintenance];
    let mut violations: Vec<String> = doors
        .iter()
        .flat_map(|door| {
            door.entries(file)
                .iter()
                .filter(|entry| entry.reason.trim().is_empty())
                .map(move |entry| {
                    format!(
                        "architecture.toml: {} `{}` needs a reason",
                        door.list().replace('_', " "),
                        entry.path
                    )
                })
        })
        .collect();
    let mut used: BTreeSet<(Door, &str)> = BTreeSet::new();
    for (path, text) in sources {
        if path.starts_with("crates/kernel/") {
            continue;
        }
        let listed = |door: Door| door.entries(file).iter().any(|entry| entry.path == *path);
        violations.extend(names_a_setting(path, text, listed));
        if is_test_file(path) {
            continue;
        }
        let code = without_comments(without_tests(text));
        let found: BTreeSet<&str> = IDENTIFIER
            .find_iter(&code)
            .map(|name| name.as_str())
            .collect();
        for (name, door) in SCOPE_IDENTIFIERS {
            if !found.contains(name) {
                continue;
            }
            if door == Door::Never {
                violations.push(format!(
                    "{path}: uses `{name}`; only the kernel names a scope's setting (open it \
                     with `kernel::db::begin_kernel` / `begin_maintenance` / `begin_admin`)"
                ));
            } else if listed(door) {
                used.insert((door, path.as_str()));
            } else {
                violations.push(format!(
                    "{path}: uses `{name}`, which opens the {} (cross-workspace access); \
                     work in one workspace (`begin_for`, the job's `ctx.tx()`), or list the \
                     file under `[[access.{}]]` with the reason (reviewed like an ADR)",
                    door.list().replace('_', " "),
                    door.list()
                ));
            }
        }
    }
    for door in doors {
        for entry in door.entries(file) {
            if !used.contains(&(door, entry.path.as_str())) {
                violations.push(format!(
                    "architecture.toml: {} `{}` names no file that opens it",
                    door.list().replace('_', " "),
                    entry.path
                ));
            }
        }
    }
    violations
}

/// `path` (outside the kernel) names a scope's setting (`campbooks.…`,
/// anywhere, any case), or, unless `listed` for that scope, its last word on
/// its own in code or a string: the setting's name pieced together
/// (`'campbooks.' || 'kernel_scope'`, `campbooks."kernel_scope"`, `concat!`,
/// `stringify!`) still names the word.
fn names_a_setting(path: &str, text: &str, listed: impl Fn(Door) -> bool) -> Vec<String> {
    static WORDS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
        SCOPE_SETTINGS
            .iter()
            .map(|&(_, word, ..)| {
                Regex::new(&format!(r"\b{word}\b"))
                    .unwrap_or_else(|e| unreachable!("static regex compiles: {e}"))
            })
            .collect()
    });
    // As written, comments included, and with every literal's escapes
    // decoded (`"kernel\x5fscope"`, `"kernel\u{5f}scope"`, a line
    // continuation), comments dropped.
    let lowered = text.to_lowercase();
    let decoded = decoded(text).to_lowercase();
    let mut violations = Vec::new();
    for ((setting, word, door, opener), word_regex) in SCOPE_SETTINGS.iter().zip(WORDS.iter()) {
        if lowered.contains(setting) || decoded.contains(setting) {
            violations.push(format!(
                "{path}: names `{setting}`; only the kernel may (use \
                 `kernel::db::{opener}`)"
            ));
            continue;
        }
        if listed(*door) {
            continue;
        }
        if word_regex.is_match(&decoded) {
            violations.push(format!(
                "{path}: names `{word}` (in code or a string), which builds a scope's \
                 setting; only the kernel may"
            ));
        }
    }
    violations
}

/// What a stretch of Rust source is, for the text checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Lexeme {
    /// Anything else.
    Code,
    /// A line comment (up to, not including, its newline).
    LineComment,
    /// A block comment, nested ones included.
    BlockComment,
    /// A string (plain, byte, C, raw, byte raw, C raw) or character literal.
    Literal,
}

/// `code` cut into code, comments and literals, as byte ranges in order.
/// Good enough for the checks, not a full lexer: a lifetime is code, a
/// character literal is `'x'` or `'\…'`.
fn lex(code: &str) -> Vec<(Lexeme, std::ops::Range<usize>)> {
    let chars: Vec<(usize, char)> = code.char_indices().collect();
    let at = |i: usize| chars.get(i).map(|&(_, c)| c);
    let offset = |i: usize| chars.get(i).map_or(code.len(), |&(o, _)| o);
    let mut out: Vec<(Lexeme, std::ops::Range<usize>)> = Vec::new();
    let mut push = |kind: Lexeme, from: usize, to: usize| {
        let range = offset(from)..offset(to);
        if range.is_empty() {
            return;
        }
        match out.last_mut() {
            Some((Lexeme::Code, last)) if kind == Lexeme::Code && last.end == range.start => {
                last.end = range.end;
            }
            _ => out.push((kind, range)),
        }
    };
    let mut i = 0;
    while i < chars.len() {
        let (c, next) = (chars[i].1, at(i + 1));
        let end = match c {
            '/' if next == Some('/') => {
                let mut j = i;
                while j < chars.len() && chars[j].1 != '\n' {
                    j += 1;
                }
                push(Lexeme::LineComment, i, j);
                j
            }
            '/' if next == Some('*') => {
                let mut depth = 0usize;
                let mut j = i;
                while j < chars.len() {
                    if chars[j].1 == '/' && at(j + 1) == Some('*') {
                        depth += 1;
                        j += 2;
                    } else if chars[j].1 == '*' && at(j + 1) == Some('/') {
                        depth -= 1;
                        j += 2;
                        if depth == 0 {
                            break;
                        }
                    } else {
                        j += 1;
                    }
                }
                let j = j.min(chars.len());
                push(Lexeme::BlockComment, i, j);
                j
            }
            'r' | 'b' | 'c' if raw_string_at(&chars, i) => {
                let mut j = i + 1;
                if at(j) == Some('r') {
                    j += 1;
                }
                let mut hashes = 0;
                while at(j) == Some('#') {
                    hashes += 1;
                    j += 1;
                }
                j += 1;
                while j < chars.len()
                    && !(chars[j].1 == '"' && (1..=hashes).all(|k| at(j + k) == Some('#')))
                {
                    j += 1;
                }
                let j = (j + 1 + hashes).min(chars.len());
                push(Lexeme::Literal, i, j);
                j
            }
            '"' => {
                let mut j = i + 1;
                while j < chars.len() && chars[j].1 != '"' {
                    j += if chars[j].1 == '\\' { 2 } else { 1 };
                }
                let j = (j + 1).min(chars.len());
                push(Lexeme::Literal, i, j);
                j
            }
            '\'' if next == Some('\\') || at(i + 2) == Some('\'') => {
                // A character literal (`'"'`, `'\n'`), not a lifetime.
                let mut j = i + 1;
                if at(j) == Some('\\') {
                    j += 2;
                }
                while j < chars.len() && chars[j].1 != '\'' {
                    j += 1;
                }
                let j = (j + 1).min(chars.len());
                push(Lexeme::Literal, i, j);
                j
            }
            _ => {
                push(Lexeme::Code, i, i + 1);
                i + 1
            }
        };
        i = end;
    }
    out
}

/// `code` without its comments (line, trailing and block comments), string
/// and character literals kept as they are.
pub(crate) fn without_comments(code: &str) -> String {
    let mut out = String::with_capacity(code.len());
    for (kind, range) in lex(code) {
        match kind {
            Lexeme::Code | Lexeme::Literal => out.push_str(&code[range]),
            Lexeme::BlockComment => out.push(' '),
            Lexeme::LineComment => {}
        }
    }
    out
}

/// `code` without its comments, every non-raw string and character
/// literal with its escapes decoded (`\x5f`, `\u{5f}`, `\n`, a `\` line
/// continuation): a name spelled with escapes reads as the name.
fn decoded(code: &str) -> String {
    let mut out = String::with_capacity(code.len());
    for (kind, range) in lex(code) {
        let text = &code[range];
        match kind {
            Lexeme::Literal if !is_raw(text) => out.push_str(&unescape(text)),
            Lexeme::Code | Lexeme::Literal => out.push_str(text),
            Lexeme::BlockComment => out.push(' '),
            Lexeme::LineComment => {}
        }
    }
    out
}

/// Whether a literal is raw (`r"…"`, `br#"…"#`, `cr"…"`): no escapes.
fn is_raw(literal: &str) -> bool {
    let rest = literal.strip_prefix(['b', 'c']).unwrap_or(literal);
    rest.starts_with('r')
}

/// `literal` with its escapes decoded; one that does not decode is kept as
/// written.
fn unescape(literal: &str) -> String {
    let mut out = String::with_capacity(literal.len());
    let mut chars = literal.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('0') => out.push('\0'),
            Some('x') => {
                let hex: String = chars.by_ref().take(2).collect();
                if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                    out.push(char::from(byte));
                } else {
                    out.push_str("\\x");
                    out.push_str(&hex);
                }
            }
            Some('u') if chars.peek() == Some(&'{') => {
                chars.next();
                let hex: String = chars.by_ref().take_while(|&c| c != '}').collect();
                let digits: String = hex.chars().filter(|&c| c != '_').collect();
                if let Some(decoded) = u32::from_str_radix(&digits, 16)
                    .ok()
                    .and_then(char::from_u32)
                {
                    out.push(decoded);
                } else {
                    out.push_str("\\u{");
                    out.push_str(&hex);
                    out.push('}');
                }
            }
            // A line continuation: the newline and the next line's leading
            // whitespace are not part of the string.
            Some('\n') => {
                while chars.peek().is_some_and(|c| c.is_whitespace()) {
                    chars.next();
                }
            }
            Some(other) => out.push(other),
            None => out.push('\\'),
        }
    }
    out
}

/// `code` with every comment and literal blanked (spaces, newlines kept),
/// byte for byte as long as `code`: positions found in it are positions in
/// `code`.
fn code_only(code: &str) -> String {
    let mut out = String::with_capacity(code.len());
    for (kind, range) in lex(code) {
        let text = &code[range];
        if kind == Lexeme::Code {
            out.push_str(text);
            continue;
        }
        for c in text.chars() {
            if c == '\n' {
                out.push('\n');
            } else {
                out.extend(std::iter::repeat_n(' ', c.len_utf8()));
            }
        }
    }
    out
}

/// Whether a raw string (`r"…"`, `r#"…"#`, `br"…"`, `cr#"…"#`) starts at
/// `i`.
fn raw_string_at(chars: &[(usize, char)], i: usize) -> bool {
    let at = |j: usize| chars.get(j).map(|&(_, c)| c);
    if i > 0 && at(i - 1).is_some_and(|c| c.is_alphanumeric() || c == '_') {
        return false;
    }
    let mut j = i + 1;
    if matches!(at(i), Some('b' | 'c')) {
        if at(j) != Some('r') {
            return false;
        }
        j += 1;
    }
    while at(j) == Some('#') {
        j += 1;
    }
    at(j) == Some('"')
}

pub(crate) fn is_test_file(path: &str) -> bool {
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
    fn no_comment_and_no_read_order_decides_the_handler() {
        let c = catalogue();
        let s = spec(Some(json!({"resource": "mail.thread", "action": "read"})));
        let declared = "#[utoipa::path(operation_id = \"mail.list\")]\nfn handler() {}\n";
        let asks = "fn h() { authz::require_type(\"mail.thread\"); }\n";
        let file = AccessFile::default();
        let check = |sources: &[(String, String)]| check_authorize_calls(&s, &c, sources, &file);
        let at = |path: &str, text: &str| (path.to_owned(), text.to_owned());
        let real_path = "crates/modules/mail/src/list.rs";
        let real = at(real_path, &format!("{declared}{asks}"));
        let silent = at(real_path, declared);
        // A file whose docs quote the declaration is not its handler,
        // whichever is read first; nor is one quoting it in a block comment.
        let quoted = at(
            "crates/kernel/src/operations.rs",
            "//! ```\n//!     operation_id = \"mail.list\",\n//! ```\n",
        );
        let block = at(
            "crates/kernel/src/block.rs",
            "/* operation_id = \"mail.list\",\n */\nfn f() {}\n",
        );
        // A file listing the id as a literal (`"mail.list",`), sorting
        // before the real handler and asking the check itself: path order
        // alone would pick it and pass; only the rank (a real declaration
        // beats a literal) reads the real handler.
        let listed = at(
            "crates/a/src/ids.rs",
            &format!("const IDS: &[&str] = &[\n    \"mail.list\",\n];\n{asks}"),
        );
        for order in [
            vec![quoted.clone(), block.clone(), listed.clone(), real.clone()],
            vec![real.clone(), listed.clone(), block.clone(), quoted.clone()],
            vec![listed.clone(), real.clone(), quoted.clone(), block.clone()],
        ] {
            assert!(check(&order[..]).is_empty(), "{order:?}");
        }
        // A real handler that never asks fails, named, whatever is read first
        // and whatever else quotes or lists it.
        for order in [
            vec![
                listed.clone(),
                silent.clone(),
                quoted.clone(),
                block.clone(),
            ],
            vec![
                silent.clone(),
                block.clone(),
                quoted.clone(),
                listed.clone(),
            ],
        ] {
            let violations = check(&order[..]);
            assert_eq!(violations.len(), 1, "{violations:?}");
            assert!(violations[0].starts_with(real_path), "{violations:?}");
        }
        // Two real declarations are ambiguous, not settled by path; one
        // under `#[cfg(test)]` is not a declaration.
        let twin = at("crates/a/src/list.rs", &format!("{declared}{asks}"));
        let violations = check(&[twin, real.clone()][..]);
        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(
            violations[0].contains("more than one file"),
            "{violations:?}"
        );
        assert!(violations[0].contains("crates/a/src/list.rs, crates/modules/mail/src/list.rs"));
        let in_tests = at(
            "crates/a/src/list.rs",
            &format!("fn f() {{}}\n#[cfg(test)]\nmod tests {{\n{declared}}}\n"),
        );
        assert!(check(&[in_tests.clone(), real][..]).is_empty());
        assert_eq!(check(&[in_tests, silent][..]).len(), 1);
        // A commented-out call is not a call: trailing or block comment.
        for hidden in [
            format!("{declared}fn h() {{ /* authz::require_type(\"mail.thread\"); */ }}\n"),
            format!("{declared}fn h() {{ let _ = 1; // authz::require_type(\"mail.thread\")\n}}\n"),
        ] {
            assert_eq!(check(&[at(real_path, &hidden)][..]).len(), 1);
        }
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
    fn the_kernel_scope_cannot_be_reached_by_another_name() {
        let module = "crates/modules/mail/src/dispatch.rs";
        let source = |text: &str| vec![(module.to_owned(), text.to_owned())];
        let allowed = AccessFile {
            kernel_scope: vec![PoolAccess {
                path: module.to_owned(),
                reason: "the scheduler".to_owned(),
            }],
            ..AccessFile::default()
        };
        let none = AccessFile::default();
        for bypass in [
            // The setting's constant, set by hand.
            r#"sqlx::query_scalar!("SELECT set_config($1, 'on', true)", campbooks_kernel::db::KERNEL_SCOPE_SETTING)"#,
            // A rename.
            "use campbooks_kernel::db::begin_kernel as open;\nlet tx = open(&pool).await?;",
            // UFCS.
            "let tx = JobContext::kernel_tx(ctx).await?;",
            // A function value.
            "let open = campbooks_kernel::db::begin_kernel;\nlet tx = open(&pool).await?;",
            // After a string holding `//` on the same line.
            "let url = \"http://x\"; let open = begin_kernel;",
        ] {
            let found = check_kernel_scope(&source(bypass), &none);
            assert_eq!(found.len(), 1, "{bypass}: {found:?}");
        }
        // The constant stays the kernel's even in a reviewed file (whose
        // entry is then stale); the admin setting's too.
        let set_by_hand =
            r#"sqlx::query_scalar!("SELECT set_config($1, 'on', true)", db::KERNEL_SCOPE_SETTING)"#;
        assert_eq!(check_kernel_scope(&source(set_by_hand), &allowed).len(), 2);
        let admin = "let s = campbooks_kernel::db::ADMIN_READS_SETTING;";
        let found = check_kernel_scope(&source(admin), &none);
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].contains("ADMIN_READS_SETTING"));
        // Comments are not code; test code is exempt from the identifier rule.
        let commented = "// see begin_kernel\n/* kernel_tx */ fn f() {} // KERNEL_SCOPE_SETTING";
        assert!(check_kernel_scope(&source(commented), &none).is_empty());
        let tested =
            "fn f() {}\n#[cfg(test)]\nmod tests {\n    fn t() { let _ = db::begin_kernel; }\n}\n";
        assert!(check_kernel_scope(&source(tested), &none).is_empty());
    }

    #[test]
    fn the_maintenance_scope_has_its_own_door() {
        let entry = |path: &str| PoolAccess {
            path: path.to_owned(),
            reason: "the scheduler".to_owned(),
        };
        let module = "crates/modules/mail/src/dispatch.rs";
        let source = |text: &str| vec![(module.to_owned(), text.to_owned())];
        let none = AccessFile::default();
        let opens = "let tx = db::begin_maintenance(&pool).await?;";
        let job = "let tx = ctx.maintenance_tx().await?;";
        for text in [opens, job] {
            let found = check_kernel_scope(&source(text), &none);
            assert_eq!(found.len(), 1, "{text}: {found:?}");
            assert!(
                found[0].contains("[[access.maintenance_scope]]"),
                "{found:?}"
            );
        }
        let maintenance = AccessFile {
            maintenance_scope: vec![entry(module)],
            ..AccessFile::default()
        };
        assert!(check_kernel_scope(&source(opens), &maintenance).is_empty());
        assert!(check_kernel_scope(&source(job), &maintenance).is_empty());
        // The maintenance door does not open the kernel scope, nor the other
        // way round.
        let kernel_call = "let tx = db::begin_kernel(&pool).await?;";
        let found = check_kernel_scope(&source(&format!("{opens}\n{kernel_call}")), &maintenance);
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].contains("`begin_kernel`"), "{found:?}");
        let kernel = AccessFile {
            kernel_scope: vec![entry(module)],
            ..AccessFile::default()
        };
        let found = check_kernel_scope(&source(&format!("{kernel_call}\n{opens}")), &kernel);
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].contains("`begin_maintenance`"), "{found:?}");
        // Its setting, constant and word stay the kernel's, even in a
        // reviewed file.
        for named in [
            r#"sqlx::query("SELECT set_config('campbooks.maintenance_scope', 'on', true)")"#,
            r#"sqlx::query_scalar!("SELECT set_config($1, 'on', true)", db::MAINTENANCE_SCOPE_SETTING)"#,
        ] {
            let found = check_kernel_scope(&source(&format!("{opens}\n{named}")), &maintenance);
            assert_eq!(found.len(), 1, "{named}: {found:?}");
        }
        let pieced =
            r#"sqlx::query("SELECT set_config('campbooks.' || 'maintenance_scope', 'on', true)")"#;
        let found = check_kernel_scope(&source(pieced), &none);
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].contains("`maintenance_scope`"), "{found:?}");
        // A stale entry is a violation.
        let stale = check_kernel_scope(&source("fn nothing() {}"), &maintenance);
        assert_eq!(stale.len(), 1, "{stale:?}");
        assert!(stale[0].contains("maintenance scope"), "{stale:?}");
    }

    #[test]
    fn c_raw_strings_are_strings() {
        let module = "crates/modules/mail/src/dispatch.rs";
        let source = |text: &str| vec![(module.to_owned(), text.to_owned())];
        let none = AccessFile::default();
        for bypass in [
            // A `//` inside a C raw string is not a comment.
            "let s = cr#\"x\"//\"#; begin_kernel(pool);",
            "let s = cr\"//\"; let open = begin_kernel;",
            // Nor inside a byte raw string, as before.
            "let s = br#\"x\"//\"#; begin_kernel(pool);",
        ] {
            let found = check_kernel_scope(&source(bypass), &none);
            assert_eq!(found.len(), 1, "{bypass}: {found:?}");
        }
        assert_eq!(
            without_comments("let s = cr#\"a // b\"#; x // y"),
            "let s = cr#\"a // b\"#; x "
        );
        // An identifier ending in `c` or `b` before a string is no prefix.
        assert_eq!(without_comments("f(abc\"//\"); g // h"), "f(abc\"//\"); g ");
    }

    #[test]
    fn a_setting_name_built_at_runtime_still_names_the_word() {
        let module = "crates/modules/mail/src/dispatch.rs";
        let source = |path: &str, text: &str| vec![(path.to_owned(), text.to_owned())];
        let none = AccessFile::default();
        for bypass in [
            r#"sqlx::query("SELECT set_config('campbooks.' || 'kernel_scope', 'on', true)")"#,
            r#"sqlx::query("SET LOCAL campbooks.\"kernel_scope\" = 'on'")"#,
            r#"sqlx::query("SET LOCAL campbooks.KERNEL_SCOPE = 'on'")"#,
            r#"let name = concat!("campbooks.", stringify!(kernel_scope));"#,
            r#"let name = format!("campbooks.{}", "kernel_scope");"#,
            "let r#kernel_scope = 1;",
        ] {
            for path in [
                module,
                "crates/server/src/x.rs",
                "crates/modules/mail/src/tests.rs",
            ] {
                let found = check_kernel_scope(&source(path, bypass), &none);
                assert_eq!(found.len(), 1, "{path}: {bypass}: {found:?}");
                assert!(found[0].contains("kernel_scope`"), "{found:?}");
            }
        }
        // Longer names, comments and the kernel itself are fine.
        for fine in [
            "let pool = testing::kernel_scope_pool(&db).await;",
            "let s = testing::kernel_scope_setting();",
            "// the kernel_scope setting\nfn f() {}",
            "/* \"kernel_scope\" */ fn f() {}",
        ] {
            assert!(
                check_kernel_scope(&source(module, fine), &none).is_empty(),
                "{fine}"
            );
        }
        let in_kernel = r#"let s = format!("campbooks.{}", "kernel_scope");"#;
        assert!(
            check_kernel_scope(&source("crates/kernel/src/db.rs", in_kernel), &none).is_empty()
        );
        // A reviewed file may (it may open the scope anyway).
        let allowed = AccessFile {
            kernel_scope: vec![PoolAccess {
                path: module.to_owned(),
                reason: "the scheduler".to_owned(),
            }],
            ..AccessFile::default()
        };
        let reviewed = format!("{in_kernel}\nlet tx = db::begin_kernel(&pool).await?;");
        assert!(check_kernel_scope(&source(module, &reviewed), &allowed).is_empty());
    }

    #[test]
    fn test_module_text_in_a_string_or_comment_does_not_end_the_check() {
        let module = "crates/modules/mail/src/dispatch.rs";
        let source = |text: &str| vec![(module.to_owned(), text.to_owned())];
        let none = AccessFile::default();
        let tables = BTreeSet::from(["mail.threads".to_owned()]);
        for hiding in [
            "let s = \"#[cfg(test)]\nmod tests {\";\n",
            "let s = r#\"#[cfg(test)]\nmod tests {\"#;\n",
            "/* #[cfg(test)]\nmod tests { */\n",
            "// #[cfg(test)]\n// mod tests {\n",
        ] {
            let opens = format!("{hiding}fn f() {{ let open = begin_kernel; }}\n");
            let found = check_kernel_scope(&source(&opens), &none);
            assert_eq!(found.len(), 1, "{hiding}: {found:?}");
            let begins = format!("{hiding}fn f() {{ let tx = state.db.begin().await?; }}\n");
            let found = check_workspace_queries(&source(&begins), &tables, &none);
            assert_eq!(found.len(), 1, "{hiding}: {found:?}");
            assert_eq!(without_tests(&begins), begins, "{hiding}");
        }
        // A real test module still ends it, multi-byte text before it included.
        let real =
            "// café ☕\nfn f() {}\n#[cfg(test)]\nmod tests {\n    fn t() { begin_kernel; }\n}\n";
        assert_eq!(without_tests(real), "// café ☕\nfn f() {}\n");
        assert!(check_kernel_scope(&source(real), &none).is_empty());
    }

    #[test]
    fn escapes_inside_a_literal_still_name_the_setting() {
        let module = "crates/modules/mail/src/dispatch.rs";
        let source = |text: &str| vec![(module.to_owned(), text.to_owned())];
        let none = AccessFile::default();
        for (bypass, named) in [
            (
                r#"let s = "campbooks.kernel\x5fscope";"#,
                "campbooks.kernel_scope",
            ),
            (
                r#"let s = "campbooks.kernel\u{5f}scope";"#,
                "campbooks.kernel_scope",
            ),
            (
                r#"let s = "campbooks.maintenance\u{005F}scope";"#,
                "campbooks.maintenance_scope",
            ),
            (
                r#"let s = b"campbooks.admin\x5Freads";"#,
                "campbooks.admin_reads",
            ),
            (
                "let s = \"campbooks.kernel\\\n        _scope\";",
                "campbooks.kernel_scope",
            ),
            (
                r#"let s = format!("campbooks.{}", "kernel\x5fscope");"#,
                "`kernel_scope`",
            ),
            (
                r#"let s = format!("campbooks.{}", "admin\u{5f}reads");"#,
                "`admin_reads`",
            ),
        ] {
            let found = check_kernel_scope(&source(bypass), &none);
            assert_eq!(found.len(), 1, "{bypass}: {found:?}");
            assert!(found[0].contains(named), "{bypass}: {found:?}");
        }
        // A raw string has no escapes: `\x5f` there is four characters.
        assert!(check_kernel_scope(&source(r#"let s = r"kernel\x5fscope";"#), &none).is_empty());
        assert_eq!(unescape(r#""a\x5fb\u{5f}c\n\"""#), "\"a_b_c\n\"\"");
        assert_eq!(unescape(r#""bad\xzz""#), r#""bad\xzz""#);
    }

    #[test]
    fn the_admin_reads_setting_is_a_bare_word_too() {
        let source = |path: &str, text: &str| vec![(path.to_owned(), text.to_owned())];
        let none = AccessFile::default();
        for bypass in [
            r#"sqlx::query("SELECT set_config('campbooks.' || 'admin_reads', 'on', true)")"#,
            r#"let name = concat!("campbooks.", stringify!(admin_reads));"#,
            r#"sqlx::query("SET LOCAL campbooks.\"ADMIN_READS\" = 'on'")"#,
        ] {
            for path in [
                "crates/modules/mail/src/x.rs",
                "crates/server/src/admin_tests.rs",
            ] {
                let found = check_kernel_scope(&source(path, bypass), &none);
                assert_eq!(found.len(), 1, "{path}: {bypass}: {found:?}");
                assert!(found[0].contains("`admin_reads`"), "{found:?}");
            }
        }
        let named = r#"sqlx::query("SELECT set_config('campbooks.admin_reads', 'on', true)")"#;
        let found = check_kernel_scope(&source("crates/modules/mail/src/x.rs", named), &none);
        assert!(found[0].contains("kernel::db::begin_admin"), "{found:?}");
        // A longer name, and the kernel, are fine.
        for (path, fine) in [
            (
                "crates/server/src/admin_tests.rs",
                "async fn admin_reads_reach_only() {}",
            ),
            (
                "crates/kernel/src/db.rs",
                "const S: &str = \"campbooks.admin_reads\";",
            ),
        ] {
            assert!(
                check_kernel_scope(&source(path, fine), &none).is_empty(),
                "{fine}"
            );
        }
    }

    #[test]
    fn only_the_kernel_names_the_idempotency_caller() {
        let source = |path: &str, text: &str| vec![(path.to_owned(), text.to_owned())];
        let none = AccessFile::default();
        for (bypass, named) in [
            (
                r#"sqlx::query("SELECT set_config('campbooks.idempotency_caller', 'user:a', true)")"#,
                "kernel::db::begin_as_idempotency_caller",
            ),
            (
                r#"sqlx::query("SELECT set_config('campbooks.' || 'idempotency_caller', $1, true)")"#,
                "`idempotency_caller`",
            ),
            (
                "let s = campbooks_kernel::db::IDEMPOTENCY_CALLER_SETTING;",
                "IDEMPOTENCY_CALLER_SETTING",
            ),
        ] {
            let found = check_kernel_scope(&source("crates/modules/mail/src/x.rs", bypass), &none);
            assert_eq!(found.len(), 1, "{bypass}: {found:?}");
            assert!(found[0].contains(named), "{bypass}: {found:?}");
        }
        for (path, fine) in [
            (
                "crates/server/src/lib.rs",
                "let name = testing::idempotency_caller_setting();",
            ),
            (
                "crates/kernel/src/db.rs",
                "const S: &str = \"campbooks.idempotency_caller\";",
            ),
        ] {
            assert!(
                check_kernel_scope(&source(path, fine), &none).is_empty(),
                "{fine}"
            );
        }
    }

    #[test]
    fn only_a_trailing_top_level_test_module_ends_the_check() {
        let module = "crates/modules/mail/src/dispatch.rs";
        let source = |text: &str| vec![(module.to_owned(), text.to_owned())];
        let none = AccessFile::default();
        let opens = "fn f() { let open = begin_kernel; }\n";
        for hiding in [
            // Inside another module.
            format!("mod inner {{\n#[cfg(test)]\nmod tests {{\n}}\n}}\n{opens}"),
            // Inside a macro's body, braces or parentheses.
            format!("macro_rules! m {{ () => {{\n#[cfg(test)]\nmod tests {{}}\n}} }}\n{opens}"),
            format!("macro_rules! m (\n#[cfg(test)]\nmod tests {{}}\n);\n{opens}"),
            // Followed by more code.
            format!("#[cfg(test)]\nmod tests {{\n}}\n{opens}"),
            format!("#[cfg(test)]\nmod tests {{ fn t() {{ }} }} {opens}"),
        ] {
            assert_eq!(without_tests(&hiding), hiding, "{hiding}");
            let found = check_kernel_scope(&source(&hiding), &none);
            assert_eq!(found.len(), 1, "{hiding}: {found:?}");
        }
        // The real one ends it: last, at the top, comments or other
        // attributes around it included.
        for real in [
            "fn f() {}\n#[cfg(test)]\nmod tests {\n    fn t() { begin_kernel; }\n}\n",
            "fn f() {}\n#[cfg(test)]\n#[expect(clippy::x)]\nmod tests { fn t() { begin_kernel; } }\n// end\n",
            "fn f() {}\n#[cfg(test)]\nmod  tests  {\n    mod nested { fn t() { begin_kernel; } }\n}",
        ] {
            assert_eq!(without_tests(real), "fn f() {}\n", "{real}");
            assert!(
                check_kernel_scope(&source(real), &none).is_empty(),
                "{real}"
            );
        }
        // Not a test module by that name: no cut.
        let other = "fn f() {}\n#[cfg(test)]\nmod testsuite { fn t() { begin_kernel; } }\n";
        assert_eq!(without_tests(other), other);
    }

    #[test]
    fn a_new_bff_file_is_checked() {
        let tables = BTreeSet::new();
        let entry = |path: &str| PoolAccess {
            path: path.to_owned(),
            reason: "instance-wide sessions".to_owned(),
        };
        let file = AccessFile {
            pool_access: vec![
                entry("crates/server/src/bff/store.rs"),
                entry("crates/server/src/bff/mod.rs"),
            ],
            ..AccessFile::default()
        };
        let opens = "let tx = self.db.begin().await?;";
        let check = |path: &str| {
            check_workspace_queries(&[(path.to_owned(), opens.to_owned())], &tables, &file)
        };
        assert!(check("crates/server/src/bff/store.rs").is_empty());
        assert_eq!(check("crates/server/src/bff/sessions.rs").len(), 1);
        // The repository's own entries name files, never the BFF's directory.
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap_or_else(|| unreachable!("xtask lives in the repository"));
        let text = std::fs::read_to_string(root.join("architecture.toml"))
            .unwrap_or_else(|e| panic!("reading architecture.toml: {e}"));
        let parsed: crate::arch::ArchitectureFile =
            toml::from_str(&text).unwrap_or_else(|e| panic!("parsing architecture.toml: {e}"));
        let directories: Vec<&str> = parsed
            .access
            .pool_access
            .iter()
            .map(|entry| entry.path.as_str())
            .filter(|path| path.starts_with("crates/server/") && path.ends_with('/'))
            .collect();
        assert!(directories.is_empty(), "{directories:?}");
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
