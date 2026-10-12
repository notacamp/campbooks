//! `cargo xtask openapi [--check]` — keep `openapi/openapi.json` in sync.

use std::{fs, path::Path};

use anyhow::{Context as _, Result, bail};

/// Path of the committed document, relative to the workspace root.
pub const DOCUMENT: &str = "openapi/openapi.json";

/// The document as committed: pretty JSON with a trailing newline.
pub fn render() -> Result<String> {
    let mut json = serde_json::to_string_pretty(&campbooks_server::openapi())
        .context("serializing the OpenAPI document")?;
    json.push('\n');
    Ok(json)
}

/// Write the document, or with `check` fail if the committed one is stale.
pub fn run(root: &Path, check: bool) -> Result<String> {
    let path = root.join(DOCUMENT);
    let fresh = render()?;

    if check {
        let committed = fs::read_to_string(&path).unwrap_or_default();
        if let Some(line) = first_difference(&committed, &fresh) {
            bail!(
                "{DOCUMENT} is stale (first difference at line {line}).\n\
                 Run `cargo xtask openapi` and commit the result."
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

/// 1-based line number of the first difference, if any.
fn first_difference(a: &str, b: &str) -> Option<usize> {
    if a == b {
        return None;
    }
    let mut left = a.lines();
    let mut right = b.lines();
    let mut line = 1;
    loop {
        match (left.next(), right.next()) {
            (Some(x), Some(y)) if x == y => line += 1,
            // A differing line, one side ending early, or (with every line
            // equal) a different trailing newline.
            _ => return Some(line),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_first_differing_line() {
        assert_eq!(first_difference("a\nb\n", "a\nb\n"), None);
        assert_eq!(first_difference("a\nb\n", "a\nc\n"), Some(2));
        assert_eq!(first_difference("a\n", "a\nb\n"), Some(2));
        assert_eq!(first_difference("a", "a\n"), Some(2));
    }

    #[test]
    fn render_ends_with_newline_and_is_valid_json() {
        let doc = render().unwrap();
        assert!(doc.ends_with("}\n"));
        let value: serde_json::Value = serde_json::from_str(&doc).unwrap();
        assert_eq!(value["info"]["title"], "Campbooks API");
    }
}
