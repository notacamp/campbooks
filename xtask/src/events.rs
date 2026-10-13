//! `cargo xtask events [--check]` — keep `openapi/events.json` in sync.

use std::{fs, path::Path};

use anyhow::{Context as _, Result, bail};

use crate::openapi::first_difference;

/// Path of the committed catalogue, relative to the workspace root.
pub const DOCUMENT: &str = "openapi/events.json";

/// The catalogue as committed: pretty JSON with a trailing newline.
pub fn render() -> Result<String> {
    let catalogue = campbooks_server::events_catalogue().context("building the registry")?;
    let mut json =
        serde_json::to_string_pretty(&catalogue).context("serializing the events catalogue")?;
    json.push('\n');
    Ok(json)
}

/// Write the catalogue, or with `check` fail if the committed one is stale.
pub fn run(root: &Path, check: bool) -> Result<String> {
    let path = root.join(DOCUMENT);
    let fresh = render()?;

    if check {
        let committed = fs::read_to_string(&path).unwrap_or_default();
        if let Some(line) = first_difference(&committed, &fresh) {
            bail!(
                "{DOCUMENT} is stale (first difference at line {line}).\n\
                 Run `cargo xtask events` and commit the result."
            );
        }
        return Ok(format!("{DOCUMENT} is up to date"));
    }

    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    fs::write(&path, fresh).with_context(|| format!("writing {}", path.display()))?;
    Ok(format!("wrote {DOCUMENT}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_lists_events_and_their_schemas() {
        let doc = render().unwrap();
        assert!(doc.ends_with("}\n"));
        let value: serde_json::Value = serde_json::from_str(&doc).unwrap();
        assert!(value["events"].is_array());
        assert!(value["components"]["schemas"].is_object());
    }
}
