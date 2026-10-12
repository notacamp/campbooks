//! `cargo xtask declarations [--check]` — keep `openapi/declarations.json`,
//! the catalogue of every module's record types, actions and events, in sync.
//! Clients generate their shortcut registry and command menu from it.

use std::path::Path;

use anyhow::{Context as _, Result};

use crate::generated;

/// Path of the committed catalogue, relative to the workspace root.
pub const DOCUMENT: &str = "openapi/declarations.json";

/// The catalogue as committed: pretty JSON with a trailing newline.
pub fn render() -> Result<String> {
    let catalogue = campbooks_server::catalogue().context("building the declarations catalogue")?;
    generated::pretty(&catalogue, "declarations catalogue")
}

/// Write the catalogue, or with `check` fail if the committed one is stale.
pub fn run(root: &Path, check: bool) -> Result<String> {
    generated::sync(
        root,
        DOCUMENT,
        &render()?,
        check,
        "cargo xtask declarations",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_lists_the_reference_action() {
        let doc: serde_json::Value = serde_json::from_str(&render().unwrap()).unwrap();
        let names: Vec<&str> = doc["actions"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|a| a["name"].as_str())
            .collect();
        assert!(names.contains(&"system.whoami"), "{names:?}");
    }
}
