//! The plan's rules, as pure functions over the plan (and, for the rules
//! that look at the code, over the sources given to them).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::LazyLock;

use regex::Regex;

use crate::model::{
    ACTOR_TYPES, CLASSIFICATIONS, COVERAGE_KINDS, FIXED_ACTORS, Plan, REJECTED_CLASSIFICATIONS,
    SCOPES, STATUSES, TYPES, enum_type_name, event_type_name, module_of, record_type_name,
};

static SNAKE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[a-z][a-z0-9]*(_[a-z0-9]+)*$")
        .unwrap_or_else(|e| unreachable!("static regex compiles: {e}"))
});

static QUALIFIED: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[a-z][a-z0-9_]*\.[a-z][a-z0-9]*(_[a-z0-9]+)*$")
        .unwrap_or_else(|e| unreachable!("static regex compiles: {e}"))
});

static DATE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\d{4}-(0[1-9]|1[0-2])-(0[1-9]|[12]\d|3[01])$")
        .unwrap_or_else(|e| unreachable!("static regex compiles: {e}"))
});

/// Past participles that do not end in `-ed`, accepted as an event's action.
const IRREGULAR_PAST: &[&str] = &[
    "begun",
    "bought",
    "brought",
    "built",
    "caught",
    "chosen",
    "cut",
    "dealt",
    "done",
    "drawn",
    "forgotten",
    "found",
    "frozen",
    "given",
    "held",
    "hidden",
    "kept",
    "known",
    "laid",
    "left",
    "lent",
    "let",
    "lost",
    "made",
    "met",
    "paid",
    "put",
    "quit",
    "read",
    "rebuilt",
    "redone",
    "reset",
    "rewritten",
    "run",
    "seen",
    "sent",
    "set",
    "shown",
    "shut",
    "sold",
    "split",
    "spent",
    "taken",
    "taught",
    "thrown",
    "told",
    "undone",
    "unset",
    "won",
    "written",
];

/// Names a property or field may not take: the generated struct's own
/// `workspace` field, and Rust keywords.
const RESERVED_NAMES: &[&str] = &[
    "workspace",
    "as",
    "async",
    "await",
    "break",
    "const",
    "continue",
    "crate",
    "dyn",
    "else",
    "enum",
    "extern",
    "false",
    "fn",
    "for",
    "gen",
    "if",
    "impl",
    "in",
    "let",
    "loop",
    "match",
    "mod",
    "move",
    "mut",
    "pub",
    "ref",
    "return",
    "self",
    "static",
    "struct",
    "super",
    "trait",
    "true",
    "type",
    "unsafe",
    "use",
    "where",
    "while",
    "abstract",
    "become",
    "box",
    "do",
    "final",
    "macro",
    "override",
    "priv",
    "try",
    "typeof",
    "unsized",
    "virtual",
    "yield",
];

/// Modules that own events but are no crate under `crates/modules`: their
/// events are generated into the kernel.
pub const PLATFORM_MODULES: &[&str] = &["kernel", "server"];

/// Every rule that reads the plan alone. `modules` are the module crates
/// (directory names under `crates/modules`).
pub fn plan(plan: &Plan, modules: &BTreeSet<String>) -> Vec<String> {
    let mut problems = Vec::new();
    let owns = |module: &str| modules.contains(module) || PLATFORM_MODULES.contains(&module);
    changelog(plan, &mut problems);
    actors(plan, &mut problems);
    let enums = enums(plan, &owns, &mut problems);
    records(plan, &owns, &enums, &mut problems);
    events(plan, &owns, &mut problems);
    properties(plan, &enums, &mut problems);
    coverage(plan, &mut problems);
    generated_names(plan, &enums, &mut problems);
    problems
}

/// The enums and their values.
type Enums<'a> = BTreeMap<&'a str, BTreeSet<&'a str>>;

fn changelog(plan: &Plan, problems: &mut Vec<String>) {
    if plan.changelog.is_empty() {
        problems.push("Changelog: the plan has no version yet; add a Changelog row".to_owned());
    }
    let mut seen_versions = BTreeSet::new();
    for row in &plan.changelog {
        if row.plan_version == 0 {
            problems.push("Changelog: plan versions start at 1".to_owned());
        }
        if !seen_versions.insert(row.plan_version) {
            problems.push(format!(
                "Changelog: plan version {} is listed twice",
                row.plan_version
            ));
        }
        if !DATE.is_match(&row.date) {
            problems.push(format!(
                "Changelog: plan version {}'s date `{}` is not YYYY-MM-DD",
                row.plan_version, row.date
            ));
        }
        if row.change.trim().is_empty() {
            problems.push(format!(
                "Changelog: plan version {} says nothing about what changed",
                row.plan_version
            ));
        }
    }
    let mut by_version = plan.changelog.clone();
    by_version.sort_by_key(|row| row.plan_version);
    for pair in by_version.windows(2) {
        if pair[1].date < pair[0].date {
            problems.push(format!(
                "Changelog: plan version {} is dated before version {}",
                pair[1].plan_version, pair[0].plan_version
            ));
        }
    }
}

fn actors(plan: &Plan, problems: &mut Vec<String>) {
    let actors: Vec<&str> = plan.actors.iter().map(|row| row.kind.as_str()).collect();
    let mut sorted_actors = actors.clone();
    sorted_actors.sort_unstable();
    let mut fixed: Vec<&str> = FIXED_ACTORS.iter().map(|(kind, ..)| *kind).collect();
    fixed.sort_unstable();
    if sorted_actors != fixed {
        problems.push(format!(
            "Actors: the sheet lists exactly the actor types {} (found {})",
            ACTOR_TYPES.join(", "),
            actors.join(", ")
        ));
    }
    for row in &plan.actors {
        if row.id_is.trim().is_empty() || row.description.trim().is_empty() {
            problems.push(format!(
                "Actors: `{}` needs what its id is and a description",
                row.kind
            ));
        }
    }
}

fn enums<'a>(plan: &'a Plan, owns: &dyn Fn(&str) -> bool, problems: &mut Vec<String>) -> Enums<'a> {
    let mut enums: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for row in &plan.enums {
        let at = format!("Enums: `{}` value `{}`", row.enumeration, row.value);
        if !QUALIFIED.is_match(&row.enumeration) {
            problems.push(format!("{at}: the enum is not named <module>.<name>"));
        } else if !owns(module_of(&row.enumeration)) {
            problems.push(format!("{at}: no module `{}`", module_of(&row.enumeration)));
        }
        if !SNAKE.is_match(&row.value) {
            problems.push(format!("{at}: the value is not snake_case"));
        }
        if row.description.trim().is_empty() {
            problems.push(format!("{at}: needs a description"));
        }
        if !enums
            .entry(row.enumeration.as_str())
            .or_default()
            .insert(row.value.as_str())
        {
            problems.push(format!("{at}: listed twice"));
        }
    }
    enums
}

fn records(
    plan: &Plan,
    owns: &dyn Fn(&str) -> bool,
    enums: &Enums<'_>,
    problems: &mut Vec<String>,
) {
    let owned = records_in(plan);
    let mut fields = BTreeSet::new();
    for row in &plan.records {
        let at = format!("Records: `{}` field `{}`", row.record, row.field);
        if !QUALIFIED.is_match(&row.record) {
            problems.push(format!("{at}: the record is not named <module>.<record>"));
        } else if !owns(module_of(&row.record)) {
            problems.push(format!("{at}: no module `{}`", module_of(&row.record)));
        }
        name(problems, &at, &row.field);
        if !fields.insert((row.record.as_str(), row.field.as_str())) {
            problems.push(format!("{at}: listed twice"));
        }
        if row.kind == "record" {
            problems.push(format!(
                "{at}: a record field cannot hold another record; reference it (record_ref)"
            ));
        }
        typed(
            problems,
            &at,
            &Typed {
                kind: &row.kind,
                list: row.list,
                of: &row.enumeration,
                classification: &row.classification,
                pattern: &row.pattern,
                description: &row.description,
            },
            enums,
            &owned,
            module_of(&row.record),
        );
    }
}

fn events(plan: &Plan, owns: &dyn Fn(&str) -> bool, problems: &mut Vec<String>) {
    let version = plan.version();
    let mut events = BTreeSet::new();
    let mut newest: BTreeMap<&str, u16> = BTreeMap::new();
    for row in &plan.events {
        let entry = newest.entry(row.name.as_str()).or_default();
        *entry = (*entry).max(row.version);
    }
    for row in &plan.events {
        let at = format!("Events: `{}` v{}", row.name, row.version);
        if !events.insert((row.name.as_str(), row.version)) {
            problems.push(format!("{at}: listed twice"));
        }
        if row.version == 0 {
            problems.push(format!("{at}: versions start at 1"));
        }
        if !QUALIFIED.is_match(&row.name) {
            problems.push(format!("{at}: not named <module>.<object>_<past verb>"));
        }
        let expected = format!("{}.{}_{}", row.module, row.object, row.action);
        if row.name != expected {
            problems.push(format!(
                "{at}: its name must be <module>.<object>_<action> from its columns: `{expected}`"
            ));
        }
        if !owns(&row.module) {
            problems.push(format!("{at}: no module `{}`", row.module));
        }
        if !SNAKE.is_match(&row.object) || !SNAKE.is_match(&row.action) {
            problems.push(format!("{at}: object and action are snake_case"));
        }
        if !past_tense(&row.action) {
            problems.push(format!(
                "{at}: the action `{}` is not a past participle (`archived`, `sent`)",
                row.action
            ));
        }
        one_of(problems, &at, "scope", &row.scope, SCOPES);
        one_of(problems, &at, "status", &row.status, STATUSES);
        if row.status == "live" && newest.get(row.name.as_str()) != Some(&row.version) {
            problems.push(format!(
                "{at}: only the newest version of an event may be live; deprecate this one"
            ));
        }
        if row.actor_types.is_empty() {
            problems.push(format!("{at}: name the actor types that may cause it"));
        }
        for actor in &row.actor_types {
            if !ACTOR_TYPES.contains(&actor.as_str()) {
                problems.push(format!(
                    "{at}: `{actor}` is no actor type ({})",
                    ACTOR_TYPES.join(", ")
                ));
            }
        }
        if row.description.trim().is_empty() || row.fires_when.trim().is_empty() {
            problems.push(format!("{at}: needs a description and when it fires"));
        }
        if row.added_in == 0 || row.added_in > version {
            problems.push(format!(
                "{at}: added in plan version {}, which the Changelog does not have",
                row.added_in
            ));
        }
        if row.openspec_change.trim().is_empty() {
            problems.push(format!("{at}: name the OpenSpec change that added it"));
        }
    }
}

fn properties(plan: &Plan, enums: &Enums<'_>, problems: &mut Vec<String>) {
    let mut properties = BTreeSet::new();
    let records_owned = records_in(plan);
    for row in &plan.properties {
        let at = format!(
            "Properties: `{}` v{} `{}`",
            row.event, row.version, row.property
        );
        let Some(event) = plan
            .events
            .iter()
            .find(|e| e.name == row.event && e.version == row.version)
        else {
            problems.push(format!("{at}: no such event version in Events"));
            continue;
        };
        name(problems, &at, &row.property);
        if !properties.insert((row.event.as_str(), row.version, row.property.as_str())) {
            problems.push(format!("{at}: listed twice"));
        }
        typed(
            problems,
            &at,
            &Typed {
                kind: &row.kind,
                list: row.list,
                of: &row.of,
                classification: &row.classification,
                pattern: &row.pattern,
                description: &row.description,
            },
            enums,
            &records_owned,
            &event.module,
        );
    }
}

fn coverage(plan: &Plan, problems: &mut Vec<String>) {
    let names: BTreeSet<&str> = plan.events.iter().map(|e| e.name.as_str()).collect();
    let mut covered = BTreeSet::new();
    for row in &plan.coverage {
        let at = format!("Coverage: {} `{}`", row.kind, row.name);
        one_of(problems, &at, "kind", &row.kind, COVERAGE_KINDS);
        if !covered.insert((row.kind.as_str(), row.name.as_str())) {
            problems.push(format!("{at}: listed twice"));
        }
        if !QUALIFIED.is_match(&row.name) {
            problems.push(format!("{at}: not a <module>.<name>"));
        }
        let problem = match (row.events.is_empty(), row.exemption.trim().is_empty()) {
            (true, true) => Some("list the events it tracks, or why it tracks none (Exemption)"),
            (false, false) => Some("lists events and an exemption; keep one"),
            _ => None,
        };
        if let Some(problem) = problem {
            problems.push(format!("{at}: {problem}"));
        }
        for event in &row.events {
            if !names.contains(event.as_str()) {
                problems.push(format!("{at}: no event `{event}` in Events"));
            }
        }
    }
}

fn generated_names(plan: &Plan, enums: &Enums<'_>, problems: &mut Vec<String>) {
    let mut types: BTreeMap<(String, String), String> = BTreeMap::new();
    let mut claim = |problems: &mut Vec<String>, module: &str, type_name: String, what: String| {
        let crate_of = crate_of(module).to_owned();
        if let Some(other) = types.insert((crate_of, type_name.clone()), what.clone()) {
            problems.push(format!(
                "{what} and {other} would both generate `{type_name}`; rename one"
            ));
        }
    };
    for row in &plan.events {
        claim(
            &mut *problems,
            &row.module,
            event_type_name(row),
            format!("event `{}` v{}", row.name, row.version),
        );
    }
    for record in &records_in(plan) {
        claim(
            &mut *problems,
            module_of(record),
            record_type_name(record),
            format!("record `{record}`"),
        );
    }
    for enumeration in enums.keys() {
        claim(
            &mut *problems,
            module_of(enumeration),
            enum_type_name(enumeration),
            format!("enum `{enumeration}`"),
        );
    }
}

/// The crate an owning module's generated code goes to: its own, or the
/// kernel's for the platform modules.
pub fn crate_of(module: &str) -> &str {
    if PLATFORM_MODULES.contains(&module) {
        "kernel"
    } else {
        module
    }
}

fn records_in(plan: &Plan) -> BTreeSet<&str> {
    plan.records.iter().map(|row| row.record.as_str()).collect()
}

fn past_tense(action: &str) -> bool {
    let last = action.rsplit('_').next().unwrap_or(action);
    last.ends_with("ed") || IRREGULAR_PAST.contains(&last)
}

fn name(problems: &mut Vec<String>, at: &str, name: &str) {
    if !SNAKE.is_match(name) {
        problems.push(format!("{at}: the name is not snake_case"));
    }
    if RESERVED_NAMES.contains(&name) {
        problems.push(format!("{at}: `{name}` is reserved; pick another name"));
    }
}

fn one_of(problems: &mut Vec<String>, at: &str, what: &str, value: &str, allowed: &[&str]) {
    if !allowed.contains(&value) {
        problems.push(format!(
            "{at}: {what} `{value}` is not one of {}",
            allowed.join(", ")
        ));
    }
}

/// A property's or a field's type columns.
struct Typed<'a> {
    kind: &'a str,
    list: bool,
    of: &'a str,
    classification: &'a str,
    pattern: &'a str,
    description: &'a str,
}

fn typed(
    problems: &mut Vec<String>,
    at: &str,
    field: &Typed<'_>,
    enums: &BTreeMap<&str, BTreeSet<&str>>,
    records: &BTreeSet<&str>,
    module: &str,
) {
    one_of(problems, at, "type", field.kind, TYPES);
    one_of(
        problems,
        at,
        "classification",
        field.classification,
        CLASSIFICATIONS,
    );
    if REJECTED_CLASSIFICATIONS.contains(&field.classification) {
        problems.push(format!(
            "{at}: classified `{}`: events carry no personal data or content; remove it, or \
             carry an id instead",
            field.classification
        ));
    }
    if field.description.trim().is_empty() {
        problems.push(format!("{at}: needs a description"));
    }
    match field.kind {
        "enum" => match enums.get(field.of) {
            None => problems.push(format!("{at}: no enum `{}` in Enums", field.of)),
            Some(_) if module_of(field.of) != module => problems.push(format!(
                "{at}: the enum `{}` belongs to another module than `{module}`",
                field.of
            )),
            Some(_) => {}
        },
        "record" => {
            if field.list {
                problems.push(format!("{at}: a list of records is not allowed"));
            }
            if !records.contains(field.of) {
                problems.push(format!("{at}: no record `{}` in Records", field.of));
            } else if module_of(field.of) != module {
                problems.push(format!(
                    "{at}: the record `{}` belongs to another module than `{module}`: \
                     snapshots come from the module's own rows",
                    field.of
                ));
            }
        }
        _ if !field.of.is_empty() => problems.push(format!(
            "{at}: only enum and record types name an enum or a record"
        )),
        _ => {}
    }
    if field.kind == "code" {
        if field.pattern.is_empty() {
            problems.push(format!("{at}: a code needs a pattern"));
        } else if let Err(e) = Regex::new(field.pattern) {
            problems.push(format!("{at}: the pattern does not parse: {e}"));
        } else if !field.pattern.starts_with('^') || !field.pattern.ends_with('$') {
            problems.push(format!(
                "{at}: the pattern must match the whole code (^...$)"
            ));
        }
    } else if !field.pattern.is_empty() {
        problems.push(format!("{at}: only a code has a pattern"));
    }
}

/// One Rust source file: its path relative to the repository and its text.
pub type Source = (String, String);

/// The rules that read the code (`sources`: every Rust file under
/// `crates/`): each live event is tracked outside generated code and tests
/// in its module's crate (a `server` event in the server's), a planned one
/// is not, and nothing implements `TrackedEvent` by hand.
pub fn code(plan: &Plan, sources: &[Source]) -> Vec<String> {
    let mut problems = Vec::new();
    let production: Vec<(&str, String)> = sources
        .iter()
        .filter(|(path, _)| {
            !is_generated(path) && !is_test_file(path) && !path.starts_with(TOOL_DIR)
        })
        .map(|(path, text)| (path.as_str(), without_tests(text)))
        .collect();
    for row in &plan.events {
        let type_name = event_type_name(row);
        let dir = source_dir(&row.module);
        let used = production
            .iter()
            .any(|(path, text)| path.starts_with(&dir) && contains_word(text, &type_name));
        match (row.status.as_str(), used) {
            ("live", false) => problems.push(format!(
                "event `{}` v{} is live but `{type_name}` is not tracked anywhere in {dir} \
                 outside generated code and tests",
                row.name, row.version
            )),
            ("planned", true) => problems.push(format!(
                "event `{}` v{} is planned but {dir} tracks `{type_name}`: mark it live",
                row.name, row.version
            )),
            _ => {}
        }
    }
    for (path, text) in &production {
        if IMPLEMENTING.is_match(text) {
            problems.push(format!(
                "{path} implements `TrackedEvent` by hand: only types generated from the plan \
                 are tracked; add the event to the plan and generate it"
            ));
        }
    }
    problems
}

/// A hand-written implementation of `TrackedEvent`.
static IMPLEMENTING: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\bimpl\b[^{;]*\bTrackedEvent\s+for\b")
        .unwrap_or_else(|e| unreachable!("static regex compiles: {e}"))
});

/// Whether `text` holds `word` as a whole identifier.
fn contains_word(text: &str, word: &str) -> bool {
    let identifier = |c: char| c.is_ascii_alphanumeric() || c == '_';
    text.match_indices(word).any(|(at, _)| {
        let before = text[..at].chars().next_back();
        let after = text[at + word.len()..].chars().next();
        !before.is_some_and(identifier) && !after.is_some_and(identifier)
    })
}

/// This crate, whose generator writes `impl … TrackedEvent for` as text.
const TOOL_DIR: &str = "crates/tracking-plan/";

/// Where the code that tracks a module's events lives.
pub fn source_dir(module: &str) -> String {
    match module {
        "kernel" => "crates/kernel/".to_owned(),
        "server" => "crates/server/".to_owned(),
        module => format!("crates/modules/{module}/"),
    }
}

/// Whether `path` is generated by this tool.
pub fn is_generated(path: &str) -> bool {
    path.ends_with(".gen.rs")
}

/// Whether `path` holds only test code.
pub fn is_test_file(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    matches!(name, "tests.rs" | "testkit.rs" | "testing.rs")
        || name.ends_with("_tests.rs")
        || path.contains("/tests/")
        || path.contains("/testing/")
        || path.contains("/benches/")
}

/// `text` without its `#[cfg(test)]` items (a `mod tests { … }` block, or
/// the one item that follows).
pub fn without_tests(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find("#[cfg(test)]") {
        out.push_str(&rest[..at]);
        let after = &rest[at..];
        let Some(open) = after.find(['{', ';']) else {
            return out;
        };
        if after.as_bytes().get(open) == Some(&b';') {
            rest = &after[open + 1..];
            continue;
        }
        let mut depth = 0_usize;
        let mut end = after.len();
        for (i, c) in after[open..].char_indices() {
            match c {
                '{' => depth += 1,
                '}' => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        end = open + i + 1;
                        break;
                    }
                }
                _ => {}
            }
        }
        rest = &after[end..];
    }
    out.push_str(rest);
    out
}

/// The Coverage rows of kind `operation` whose `operationId` is in neither
/// document, and the mutating operations that no row names (warnings until
/// campbooks-cloud#332).
pub fn operations(plan: &Plan, documents: &[serde_json::Value]) -> (Vec<String>, Vec<String>) {
    let mut ids = BTreeSet::new();
    let mut mutating = BTreeSet::new();
    for document in documents {
        let Some(paths) = document.get("paths").and_then(serde_json::Value::as_object) else {
            continue;
        };
        for item in paths.values() {
            let Some(item) = item.as_object() else {
                continue;
            };
            for operation in item.values() {
                let Some(id) = operation
                    .get("operationId")
                    .and_then(serde_json::Value::as_str)
                else {
                    continue;
                };
                ids.insert(id.to_owned());
                let mutates = operation
                    .get("x-campbooks")
                    .and_then(|meta| meta.get("mutates"))
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false);
                if mutates {
                    mutating.insert(id.to_owned());
                }
            }
        }
    }
    let mut problems = Vec::new();
    let mut named = BTreeSet::new();
    for row in plan.coverage.iter().filter(|row| row.kind == "operation") {
        named.insert(row.name.clone());
        if !ids.contains(&row.name) {
            problems.push(format!(
                "Coverage: operation `{}` is in neither openapi/openapi.json nor \
                 openapi/admin.json",
                row.name
            ));
        }
    }
    let unmapped = mutating.difference(&named).cloned().collect();
    (problems, unmapped)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ChangelogRow, CoverageRow, EnumRow, EventRow, PropertyRow, RecordRow};

    fn modules() -> BTreeSet<String> {
        ["things".to_owned()].into()
    }

    /// A plan that passes every rule.
    fn good() -> Plan {
        let mut plan = Plan::empty();
        plan.changelog.push(ChangelogRow {
            date: "2026-10-23".into(),
            plan_version: 1,
            change: "First".into(),
            pr: "#1".into(),
        });
        plan.events.push(EventRow {
            name: "things.thing_made".into(),
            version: 1,
            module: "things".into(),
            object: "thing".into(),
            action: "made".into(),
            scope: "workspace".into(),
            status: "live".into(),
            actor_types: vec!["user".into()],
            description: "A thing was made.".into(),
            fires_when: "Someone makes one.".into(),
            added_in: 1,
            openspec_change: "add-things".into(),
        });
        plan.records.push(RecordRow {
            record: "things.thing".into(),
            field: "colour".into(),
            kind: "enum".into(),
            enumeration: "things.colour".into(),
            required: true,
            classification: "operational".into(),
            description: "Its colour.".into(),
            ..RecordRow::default()
        });
        plan.enums.push(EnumRow {
            enumeration: "things.colour".into(),
            value: "red".into(),
            description: "Red.".into(),
        });
        plan.properties.push(PropertyRow {
            event: "things.thing_made".into(),
            version: 1,
            property: "thing".into(),
            kind: "record".into(),
            of: "things.thing".into(),
            required: true,
            classification: "operational".into(),
            description: "The thing.".into(),
            ..PropertyRow::default()
        });
        plan.properties.push(PropertyRow {
            event: "things.thing_made".into(),
            version: 1,
            property: "reason".into(),
            kind: "code".into(),
            pattern: "^[a-z_]+$".into(),
            classification: "operational".into(),
            description: "Why.".into(),
            ..PropertyRow::default()
        });
        plan.coverage.push(CoverageRow {
            kind: "operation".into(),
            name: "things.make".into(),
            events: vec!["things.thing_made".into()],
            exemption: String::new(),
        });
        plan
    }

    fn fails(plan: &Plan, needle: &str) {
        let problems = super::plan(plan, &modules());
        assert!(
            problems.iter().any(|p| p.contains(needle)),
            "expected a problem containing `{needle}`, got {problems:?}"
        );
    }

    #[test]
    fn a_good_plan_passes() {
        assert_eq!(plan(&good(), &modules()), Vec::<String>::new());
    }

    #[test]
    fn content_in_a_snapshot_fails_naming_the_record_and_field() {
        let mut plan = good();
        plan.records.push(RecordRow {
            record: "things.thing".into(),
            field: "subject".into(),
            kind: "code".into(),
            pattern: "^.*$".into(),
            classification: "content".into(),
            description: "Its subject.".into(),
            ..RecordRow::default()
        });
        fails(
            &plan,
            "`things.thing` field `subject`: classified `content`",
        );
    }

    #[test]
    fn personal_data_fails() {
        let mut plan = good();
        plan.properties[0].classification = "personal".into();
        fails(&plan, "classified `personal`");
    }

    #[test]
    fn a_code_without_a_pattern_fails() {
        let mut plan = good();
        plan.properties[1].pattern.clear();
        fails(&plan, "a code needs a pattern");
    }

    #[test]
    fn names_follow_the_rule() {
        let mut plan = good();
        plan.events[0].action = "make".into();
        plan.events[0].name = "things.thing_make".into();
        fails(&plan, "not a past participle");
        let mut plan = good();
        plan.events[0].name = "things.thing_created".into();
        fails(&plan, "its name must be");
        let mut plan = good();
        plan.events[0].module = "nowhere".into();
        plan.events[0].name = "nowhere.thing_made".into();
        fails(&plan, "no module `nowhere`");
    }

    #[test]
    fn duplicates_fail() {
        let mut plan = good();
        plan.events.push(plan.events[0].clone());
        fails(&plan, "listed twice");
    }

    #[test]
    fn only_the_newest_version_is_live() {
        let mut plan = good();
        let mut v2 = plan.events[0].clone();
        v2.version = 2;
        plan.events.push(v2);
        fails(&plan, "only the newest version");
    }

    #[test]
    fn types_and_references_are_checked() {
        let mut plan = good();
        plan.properties[0].kind = "text".into();
        fails(&plan, "type `text` is not one of");
        let mut plan = good();
        plan.properties[0].of = "things.nothing".into();
        fails(&plan, "no record `things.nothing`");
        let mut plan = good();
        plan.records[0].enumeration = "things.shade".into();
        fails(&plan, "no enum `things.shade`");
        let mut plan = good();
        plan.properties[0].list = true;
        fails(&plan, "a list of records");
    }

    #[test]
    fn coverage_rows_list_events_or_an_exemption() {
        let mut plan = good();
        plan.coverage[0].exemption = "reads only".into();
        fails(&plan, "keep one");
        let mut plan = good();
        plan.coverage[0].events.clear();
        fails(&plan, "list the events it tracks");
        let mut plan = good();
        plan.coverage[0].events = vec!["things.thing_lost".into()];
        fails(&plan, "no event `things.thing_lost`");
    }

    #[test]
    fn actors_are_fixed() {
        let mut plan = good();
        plan.actors.pop();
        fails(&plan, "exactly the actor types");
    }

    #[test]
    fn a_stale_operation_fails_and_unmapped_ones_are_reported() {
        let plan = good();
        let document = serde_json::json!({"paths": {
            "/a": {"post": {"operationId": "things.break", "x-campbooks": {"mutates": true}}},
            "/b": {"get": {"operationId": "things.list", "x-campbooks": {"mutates": false}}},
        }});
        let (problems, unmapped) = operations(&plan, &[document]);
        assert_eq!(problems.len(), 1);
        assert!(problems[0].contains("`things.make`"));
        assert_eq!(unmapped, vec!["things.break".to_owned()]);
    }

    #[test]
    fn a_live_event_must_be_tracked_outside_tests() {
        let plan = good();
        let only_tests = vec![
            (
                "crates/modules/things/src/tracking.gen.rs".to_owned(),
                "pub struct ThingMade {}".to_owned(),
            ),
            (
                "crates/modules/things/src/make.rs".to_owned(),
                "fn f() {}\n#[cfg(test)]\nmod tests { fn t() { ThingMade {}; } }\n".to_owned(),
            ),
        ];
        let problems = code(&plan, &only_tests);
        assert!(problems[0].contains("is live but"), "{problems:?}");
        let used = vec![(
            "crates/modules/things/src/make.rs".to_owned(),
            "fn f() { track(&ThingMade {}); }".to_owned(),
        )];
        assert_eq!(code(&plan, &used), Vec::<String>::new());
        let mut planned = plan.clone();
        planned.events[0].status = "planned".into();
        assert!(code(&planned, &used)[0].contains("mark it live"));
    }

    #[test]
    fn hand_written_tracked_events_fail() {
        let plan = Plan::empty();
        let sources = vec![(
            "crates/modules/things/src/make.rs".to_owned(),
            "impl campbooks_kernel::tracking::TrackedEvent for Mine {}".to_owned(),
        )];
        assert!(code(&plan, &sources)[0].contains("by hand"));
    }

    #[test]
    fn test_code_is_cut() {
        let text = "a\n#[cfg(test)]\nmod tests {\n fn x() { { } }\n}\nb\n#[cfg(test)]\nmod t;\nc";
        assert_eq!(without_tests(text), "a\n\nb\n\nc");
    }
}
