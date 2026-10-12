//! Committed files generated from the server: write them, or check that the
//! committed copy is current.

use std::{fs, path::Path};

use anyhow::{Context as _, Result, bail};

/// `json` as committed: pretty, with a trailing newline.
pub fn pretty(value: &impl serde::Serialize, what: &str) -> Result<String> {
    let mut json =
        serde_json::to_string_pretty(value).with_context(|| format!("serializing the {what}"))?;
    json.push('\n');
    Ok(json)
}

/// Write `fresh` to `document` (relative to `root`), or with `check` fail if
/// the committed file differs, telling the reader to run `command`.
pub fn sync(
    root: &Path,
    document: &str,
    fresh: &str,
    check: bool,
    command: &str,
) -> Result<String> {
    let path = root.join(document);
    if check {
        let committed = fs::read_to_string(&path).unwrap_or_default();
        if let Some(line) = first_difference(&committed, fresh) {
            bail!(
                "{document} is stale (first difference at line {line}).\n\
                 Run `{command}` and commit the result."
            );
        }
        return Ok(format!("{document} is up to date"));
    }
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    fs::write(&path, fresh).with_context(|| format!("writing {}", path.display()))?;
    Ok(format!("wrote {document}"))
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
    fn pretty_ends_with_newline() {
        assert_eq!(
            pretty(&serde_json::json!({"a": 1}), "x").unwrap(),
            "{\n  \"a\": 1\n}\n"
        );
    }
}
