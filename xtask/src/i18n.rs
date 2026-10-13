//! `cargo xtask i18n` — every server message catalogue at parity.
//!
//! A catalogue is a directory named `i18n` under `crates/` (the kernel's,
//! the AI gateway's, each module's). Its files are held to
//! `campbooks_kernel::i18n::check_catalogue`, the rules the server itself
//! renders by. The client's catalogues are checked by `pnpm i18n:check`.

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context as _, Result, bail};
use campbooks_kernel::i18n::{Locale, check_catalogue};

/// Check every catalogue under `root/crates`.
pub fn run(root: &Path) -> Result<String> {
    let mut dirs = Vec::new();
    find_catalogues(&root.join("crates"), &mut dirs)?;
    dirs.sort();
    let mut problems = Vec::new();
    for dir in &dirs {
        let name = dir
            .strip_prefix(root)
            .unwrap_or(dir)
            .display()
            .to_string()
            .replace('\\', "/");
        problems.extend(check_catalogue(&name, &read_files(dir)?));
    }
    if problems.is_empty() {
        let names: Vec<String> = dirs
            .iter()
            .filter_map(|dir| dir.parent()?.file_name())
            .map(|name| name.to_string_lossy().into_owned())
            .collect();
        return Ok(format!(
            "i18n parity: {} server catalogue(s) [{}] x {} locales OK",
            dirs.len(),
            names.join(", "),
            Locale::ALL.len()
        ));
    }
    let list: Vec<String> = problems.iter().map(|p| format!("  - {p}")).collect();
    bail!(
        "{} i18n problem(s) (see AGENTS.md -> Internationalization):\n{}",
        problems.len(),
        list.join("\n")
    )
}

/// The `i18n` directories of the crates below `dir` (next to a
/// `Cargo.toml`; a crate's `src/i18n` is code, not a catalogue).
fn find_catalogues(dir: &Path, found: &mut Vec<PathBuf>) -> Result<()> {
    let catalogue = dir.join("i18n");
    if dir.join("Cargo.toml").is_file() && catalogue.is_dir() {
        found.push(catalogue);
    }
    for entry in fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        match entry.file_name().to_str() {
            Some("i18n" | "src" | "tests" | "target" | "node_modules") => {}
            Some(name) if name.starts_with('.') => {}
            _ => find_catalogues(&entry.path(), found)?,
        }
    }
    Ok(())
}

/// Every file of a catalogue directory, by name.
fn read_files(dir: &Path) -> Result<BTreeMap<String, String>> {
    let mut files = BTreeMap::new();
    for entry in fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let path = entry.path();
        let text =
            fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        files.insert(entry.file_name().to_string_lossy().into_owned(), text);
    }
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_repository_catalogues_are_at_parity() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let report = run(root).unwrap();
        for name in ["kernel", "ai", "system", "workspaces"] {
            assert!(report.contains(name), "{report}");
        }
    }

    #[test]
    fn drift_fails_naming_file_and_key() {
        let dir = std::env::temp_dir().join(format!("campbooks-i18n-{}", std::process::id()));
        let catalogue = dir.join("crates/modules/demo/i18n");
        fs::create_dir_all(&catalogue).unwrap();
        fs::write(dir.join("crates/modules/demo/Cargo.toml"), "").unwrap();
        for (tag, text) in [
            ("en", r#"{"errors": {"too_long": "At most {{max}}."}}"#),
            ("pt-PT", r#"{"errors": {"too_long": "No máximo {{max}}."}}"#),
            ("es-ES", r#"{"errors": {"too_long": "Como máximo."}}"#),
            ("fr", r#"{"errors": {}}"#),
        ] {
            fs::write(catalogue.join(format!("{tag}.json")), text).unwrap();
        }
        let error = run(&dir).unwrap_err().to_string();
        let _ = fs::remove_dir_all(&dir);
        assert!(
            error.contains("crates/modules/demo/i18n/fr.json: missing \"errors.too_long\""),
            "{error}"
        );
        assert!(
            error.contains(
                "crates/modules/demo/i18n/es-ES.json: \"errors.too_long\" lacks the placeholder {{max}}"
            ),
            "{error}"
        );
    }
}
