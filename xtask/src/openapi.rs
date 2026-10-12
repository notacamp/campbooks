//! `cargo xtask openapi [--check]` — keep `openapi/openapi.json` in sync.

use std::path::Path;

use anyhow::Result;

use crate::generated;

/// Path of the committed document, relative to the workspace root.
pub const DOCUMENT: &str = "openapi/openapi.json";

/// The document as committed: pretty JSON with a trailing newline.
pub fn render() -> Result<String> {
    generated::pretty(&campbooks_server::openapi(), "OpenAPI document")
}

/// Write the document, or with `check` fail if the committed one is stale.
pub fn run(root: &Path, check: bool) -> Result<String> {
    generated::sync(root, DOCUMENT, &render()?, check, "cargo xtask openapi")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_ends_with_newline_and_is_valid_json() {
        let doc = render().unwrap();
        assert!(doc.ends_with("}\n"));
        let value: serde_json::Value = serde_json::from_str(&doc).unwrap();
        assert_eq!(value["info"]["title"], "Campbooks API");
    }
}
