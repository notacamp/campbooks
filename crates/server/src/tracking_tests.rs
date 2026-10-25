//! The tracking plan's Coverage sheet against what the server really
//! composes (ADR 0020): every row names an operation of the catalogue (the
//! API's or the admin API's), or a job, schedule or event handler the
//! registry holds. The plan tool checks operations from the committed
//! documents; only the server knows its background work.
//!
//! Coverage is required, with no baseline: a mutating operation, job,
//! schedule or event handler with no row fails the test, the kernel's own
//! maintenance included. Add the row in the same PR as the work it covers
//! (`docs/tracking.md`, "Coverage").

use std::collections::BTreeSet;

use campbooks_kernel::operations::Catalogue;
use serde_json::Value;

/// The plan's mirror, as generated from the workbook.
fn plan() -> Value {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tracking/tracking-plan.json");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("reading {}: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("parsing {}: {e}", path.display()))
}

#[test]
fn coverage_names_what_the_server_composes() {
    let plan = plan();
    let registry = crate::registry().unwrap();
    let handlers: BTreeSet<&str> = registry.consumers().iter().map(|c| c.name).collect();
    let schedules: BTreeSet<&str> = registry.schedule_names().into_iter().collect();
    let jobs: BTreeSet<&str> = registry
        .kind_names()
        .into_iter()
        .filter(|kind| !handlers.contains(kind) && !schedules.contains(kind))
        .collect();
    let api = Catalogue::from_openapi(&crate::openapi());
    let admin = Catalogue::from_openapi(&crate::admin_openapi());
    let operations: Vec<_> = api.iter().chain(admin.iter()).collect();

    let rows = plan["coverage"]
        .as_array()
        .expect("the mirror lists coverage");
    let mut unknown = Vec::new();
    let mut covered = BTreeSet::new();
    for row in rows {
        let kind = row["kind"].as_str().unwrap_or_default();
        let name = row["name"].as_str().unwrap_or_default();
        let known = match kind {
            "operation" => operations.iter().any(|operation| operation.id == name),
            "job" => jobs.contains(name),
            "schedule" => schedules.contains(name),
            "handler" => handlers.contains(name),
            _ => false,
        };
        if !known {
            unknown.push(format!("{kind} `{name}`"));
        }
        covered.insert((kind.to_owned(), name.to_owned()));
    }
    assert!(
        unknown.is_empty(),
        "Coverage rows name what the server does not compose (rename or remove them in \
         tracking/tracking-plan.xlsx): {unknown:?}"
    );

    let mut missing = Vec::new();
    let mut note = |kind: &str, name: &str| {
        if !covered.contains(&(kind.to_owned(), name.to_owned())) {
            missing.push(format!("{kind} `{name}`"));
        }
    };
    for operation in operations.iter().filter(|operation| operation.mutates()) {
        note("operation", &operation.id);
    }
    for job in &jobs {
        note("job", job);
    }
    for schedule in &schedules {
        note("schedule", schedule);
    }
    for handler in &handlers {
        note("handler", handler);
    }
    assert!(
        missing.is_empty(),
        "{} mutating operation(s), job(s), schedule(s) and handler(s) have no Coverage row. \
         Add one for each to tracking/tracking-plan.xlsx, as a patch under tracking/patches/, \
         naming the events it tracks or an exemption saying why it tracks none, then run \
         `cargo run -p campbooks-tracking-plan -- apply <patch>` (docs/tracking.md, \
         \"Coverage\"):\n{}",
        missing.len(),
        missing
            .iter()
            .map(|item| format!("  - {item}"))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn the_kernel_was_generated_from_the_committed_plan() {
    assert_eq!(
        plan()["plan_version"],
        campbooks_kernel::tracking::PLAN_VERSION,
        "run `cargo run -p campbooks-tracking-plan -- generate`"
    );
}
