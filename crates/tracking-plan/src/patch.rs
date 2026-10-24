//! YAML patches: how agents and scripts change the plan without touching
//! the binary workbook. A patch adds or replaces rows (matched by their key)
//! and may remove some. Patches are kept in `tracking/patches/` as the
//! history of those edits.
//!
//! ```yaml
//! changelog:
//!   - { date: 2026-10-23, plan_version: 2, change: "Add things.thing_made", pr: "#700" }
//! events:
//!   - name: things.thing_made
//!     version: 1
//!     module: things
//!     # …every column of the Events sheet
//! properties: []
//! records: []
//! enums: []
//! coverage: []
//! actors: []   # rewords an actor type's id and description
//! remove:
//!   coverage: [{ kind: operation, name: things.old }]
//! ```

use serde::Deserialize;

use crate::model::{
    ActorRow, ChangelogRow, CoverageRow, EnumRow, EventRow, Plan, PropertyRow, RecordRow,
};

/// A patch file.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Patch {
    /// Events rows, keyed by name and version.
    pub events: Vec<EventRow>,
    /// Properties rows, keyed by event, version and property.
    pub properties: Vec<PropertyRow>,
    /// Records rows, keyed by record and field.
    pub records: Vec<RecordRow>,
    /// Enums rows, keyed by enum and value.
    pub enums: Vec<EnumRow>,
    /// Coverage rows, keyed by kind and name.
    pub coverage: Vec<CoverageRow>,
    /// Actors rows, keyed by type: what an actor type's id is and its
    /// description (the types themselves are fixed).
    pub actors: Vec<ActorRow>,
    /// Changelog rows, keyed by plan version.
    pub changelog: Vec<ChangelogRow>,
    /// Rows to remove, by key (other columns are ignored).
    pub remove: Removals,
}

/// Rows a patch removes.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Removals {
    /// `name` and `version`.
    pub events: Vec<EventRow>,
    /// `event`, `version` and `property`.
    pub properties: Vec<PropertyRow>,
    /// `record` and `field`.
    pub records: Vec<RecordRow>,
    /// `enum` and `value`.
    pub enums: Vec<EnumRow>,
    /// `kind` and `name`.
    pub coverage: Vec<CoverageRow>,
}

/// Parse a patch.
pub fn parse(text: &str) -> anyhow::Result<Patch> {
    serde_saphyr::from_str(text).map_err(|e| anyhow::anyhow!("the patch does not parse: {e}"))
}

/// Replace the row of `rows` with `row`'s key, or add it.
fn upsert<T, K: PartialEq>(rows: &mut Vec<T>, row: T, key: impl Fn(&T) -> K) {
    let wanted = key(&row);
    match rows.iter_mut().find(|existing| key(existing) == wanted) {
        Some(existing) => *existing = row,
        None => rows.push(row),
    }
}

/// Remove the rows of `rows` whose key is one of `gone`'s; returns the keys
/// that matched nothing.
fn remove<T, K: PartialEq + std::fmt::Debug>(
    rows: &mut Vec<T>,
    gone: &[T],
    key: impl Fn(&T) -> K,
) -> Vec<String> {
    let mut missing = Vec::new();
    for row in gone {
        let wanted = key(row);
        let before = rows.len();
        rows.retain(|existing| key(existing) != wanted);
        if rows.len() == before {
            missing.push(format!("{wanted:?}"));
        }
    }
    missing
}

/// Apply `patch` to `plan`. A removal that matches no row is an error, so a
/// stale patch is noticed.
pub fn apply(plan: &mut Plan, patch: Patch) -> anyhow::Result<()> {
    let mut missing = Vec::new();
    missing.extend(remove(&mut plan.events, &patch.remove.events, |r| {
        (r.name.clone(), r.version)
    }));
    missing.extend(remove(
        &mut plan.properties,
        &patch.remove.properties,
        |r| (r.event.clone(), r.version, r.property.clone()),
    ));
    missing.extend(remove(&mut plan.records, &patch.remove.records, |r| {
        (r.record.clone(), r.field.clone())
    }));
    missing.extend(remove(&mut plan.enums, &patch.remove.enums, |r| {
        (r.enumeration.clone(), r.value.clone())
    }));
    missing.extend(remove(&mut plan.coverage, &patch.remove.coverage, |r| {
        (r.kind.clone(), r.name.clone())
    }));
    if !missing.is_empty() {
        anyhow::bail!(
            "the patch removes rows the plan does not have: {}",
            missing.join(", ")
        );
    }
    for row in patch.events {
        upsert(&mut plan.events, row, |r| (r.name.clone(), r.version));
    }
    for row in patch.properties {
        upsert(&mut plan.properties, row, |r| {
            (r.event.clone(), r.version, r.property.clone())
        });
    }
    for row in patch.records {
        upsert(&mut plan.records, row, |r| {
            (r.record.clone(), r.field.clone())
        });
    }
    for row in patch.enums {
        upsert(&mut plan.enums, row, |r| {
            (r.enumeration.clone(), r.value.clone())
        });
    }
    for row in patch.coverage {
        upsert(&mut plan.coverage, row, |r| {
            (r.kind.clone(), r.name.clone())
        });
    }
    for row in patch.actors {
        upsert(&mut plan.actors, row, |r| r.kind.clone());
    }
    for row in patch.changelog {
        upsert(&mut plan.changelog, row, |r| r.plan_version);
    }
    plan.sort();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const PATCH: &str = r##"
changelog:
  - { date: "2026-10-23", plan_version: 1, change: "First", pr: "#1" }
events:
  - name: things.thing_made
    version: 1
    module: things
    object: thing
    action: made
    scope: workspace
    status: planned
    actor_types: [user, system]
    description: A thing was made.
    fires_when: Someone makes one.
    added_in: 1
    openspec_change: add-things
properties:
  - { event: things.thing_made, version: 1, property: size, type: int, required: true,
      classification: operational, description: How big. }
coverage:
  - { kind: operation, name: things.make, events: [things.thing_made] }
"##;

    #[test]
    fn a_patch_adds_then_replaces_rows() {
        let mut plan = Plan::empty();
        apply(&mut plan, parse(PATCH).unwrap()).unwrap();
        assert_eq!(plan.events.len(), 1);
        assert_eq!(plan.properties[0].kind, "int");
        assert_eq!(plan.coverage[0].events, ["things.thing_made"]);
        let again = PATCH.replace("status: planned", "status: live");
        apply(&mut plan, parse(&again).unwrap()).unwrap();
        assert_eq!(plan.events.len(), 1);
        assert_eq!(plan.events[0].status, "live");
    }

    #[test]
    fn a_patch_rewords_an_actor_type() {
        let mut plan = Plan::empty();
        let before = plan.actors.len();
        let patch =
            parse("actors:\n  - { type: client, id_is: \"its id\", description: \"An app.\" }\n")
                .unwrap();
        apply(&mut plan, patch).unwrap();
        assert_eq!(plan.actors.len(), before);
        let client = plan.actors.iter().find(|a| a.kind == "client").unwrap();
        assert_eq!(client.id_is, "its id");
    }

    #[test]
    fn removing_a_missing_row_fails() {
        let mut plan = Plan::empty();
        let patch = parse("remove:\n  coverage: [{ kind: job, name: things.gone }]\n").unwrap();
        assert!(apply(&mut plan, patch).is_err());
    }

    #[test]
    fn unknown_keys_are_refused() {
        assert!(parse("events:\n  - { name: x, colour: red }\n").is_err());
    }
}
