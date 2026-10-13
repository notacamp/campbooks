//! `cargo xtask openapi [--check]` — keep `openapi/openapi.json` (the API and
//! its operation catalogue) and `openapi/admin.json` (the admin API, ADR
//! 0015) in sync with the server's routes.

use std::{fs, path::Path};

use anyhow::{Context as _, Result, bail};

/// Path of the committed API document, relative to the workspace root.
pub const DOCUMENT: &str = "openapi/openapi.json";

/// Path of the committed admin API document, relative to the workspace root.
pub const ADMIN_DOCUMENT: &str = "openapi/admin.json";

/// A document as committed: pretty JSON with a trailing newline.
fn render_document(document: &impl serde::Serialize) -> Result<String> {
    let mut json =
        serde_json::to_string_pretty(document).context("serializing an OpenAPI document")?;
    json.push('\n');
    Ok(json)
}

/// The API document as committed.
pub fn render() -> Result<String> {
    render_document(&campbooks_server::openapi())
}

/// The admin API document as committed.
pub fn render_admin() -> Result<String> {
    render_document(&campbooks_server::admin_openapi())
}

/// Write both documents, or with `check` fail if a committed one is stale.
pub fn run(root: &Path, check: bool) -> Result<String> {
    let documents = [(DOCUMENT, render()?), (ADMIN_DOCUMENT, render_admin()?)];
    let mut done = Vec::new();
    for (name, fresh) in documents {
        let path = root.join(name);
        if check {
            let committed = fs::read_to_string(&path).unwrap_or_default();
            if let Some(line) = first_difference(&committed, &fresh) {
                bail!(
                    "{name} is stale (first difference at line {line}).\n\
                     Run `cargo xtask openapi` and commit the result."
                );
            }
            done.push(format!("{name} is up to date"));
            continue;
        }
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
        }
        fs::write(&path, fresh).with_context(|| format!("writing {}", path.display()))?;
        done.push(format!("wrote {name}"));
    }
    Ok(done.join("\n"))
}

/// 1-based line number of the first difference, if any.
pub fn first_difference(a: &str, b: &str) -> Option<usize> {
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

    #[test]
    fn the_admin_document_is_separate() {
        let admin: serde_json::Value = serde_json::from_str(&render_admin().unwrap()).unwrap();
        assert_eq!(admin["info"]["title"], "Campbooks admin API");
        assert!(admin["paths"]["/admin/v1/system/overview"].is_object());
        let api: serde_json::Value = serde_json::from_str(&render().unwrap()).unwrap();
        assert!(api["paths"]["/admin/v1/system/overview"].is_null());
    }

    #[test]
    fn check_names_the_stale_document() {
        let dir = std::env::temp_dir().join(format!("cb-openapi-{}", std::process::id()));
        fs::create_dir_all(dir.join("openapi")).unwrap();
        fs::write(dir.join(DOCUMENT), render().unwrap()).unwrap();
        fs::write(dir.join(ADMIN_DOCUMENT), "{}\n").unwrap();
        let err = run(&dir, true).unwrap_err().to_string();
        assert!(err.contains(ADMIN_DOCUMENT), "{err}");
        run(&dir, false).unwrap();
        assert!(run(&dir, true).is_ok());
        fs::remove_dir_all(dir).unwrap();
    }
}
