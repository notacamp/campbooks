//! Who may read which record types, from the catalogue.

use std::collections::{BTreeMap, BTreeSet};

use crate::{AppState, auth::Principal};

/// Which callers may see records of each type: the scopes of the read
/// operations that return it (ADR 0013 `x-campbooks.records`). The change
/// feed and search filter by it.
#[derive(Debug, Default)]
pub struct ReadRules(BTreeMap<String, Option<BTreeSet<String>>>);

impl ReadRules {
    /// The rules of the catalogue `state` carries; none (nobody sees
    /// anything) before the server installs it.
    pub fn from_state(state: &AppState) -> Self {
        let mut rules: BTreeMap<String, Option<BTreeSet<String>>> = BTreeMap::new();
        let Some(catalogue) = state.operations.catalogue() else {
            return Self(rules);
        };
        for operation in catalogue.iter().filter(|op| !op.mutates()) {
            let Some(meta) = &operation.meta else {
                continue;
            };
            for record in &meta.records {
                let rule = rules
                    .entry(record.clone())
                    .or_insert_with(|| Some(BTreeSet::new()));
                match (operation.scopes.is_empty(), rule) {
                    // A public read: anyone in the workspace may see it.
                    (true, rule) => *rule = None,
                    (false, Some(scopes)) => scopes.extend(operation.scopes.iter().cloned()),
                    (false, None) => {}
                }
            }
        }
        Self(rules)
    }

    /// Whether `principal` holds a scope of some read of `record_type`.
    /// A record type no read operation returns is shown to nobody.
    pub fn allows(&self, principal: &Principal, record_type: &str) -> bool {
        match self.0.get(record_type) {
            None => false,
            Some(None) => true,
            Some(Some(scopes)) => scopes.iter().any(|scope| principal.has_scope(scope)),
        }
    }
}
