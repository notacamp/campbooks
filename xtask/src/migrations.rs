//! `cargo xtask migrations --base <ref>`: a branch's new migrations must
//! sort after every migration already on its base.
//!
//! sqlx applies migrations in version order and refuses to start when the
//! database holds a version the files do not (or a file reuses one). Two
//! branches that each picked a timestamp, merged in either order, can leave
//! a new migration *before* one production already ran, or two files with
//! one version. `cargo xtask arch` rejects a reused version in the tree; this
//! check rejects a new migration that does not come after the base's newest.

use std::{collections::BTreeSet, fs, path::Path, process::Command};

use anyhow::{Context as _, Result, bail};

/// The version prefix of a migration file name (`<version>_<owner>__<desc>.sql`).
fn version(name: &str) -> Option<&str> {
    let (version, _) = name.split_once('_')?;
    (!version.is_empty() && version.bytes().all(|b| b.is_ascii_digit())).then_some(version)
}

/// Violations: files in `head` but not in `base` whose version does not
/// sort after every version in `base`, and versions `head` uses twice.
pub fn check_order(base: &[String], head: &[String]) -> Vec<String> {
    let mut violations = Vec::new();
    let mut seen = BTreeSet::new();
    for name in head {
        if let Some(version) = version(name)
            && !seen.insert(version)
        {
            violations.push(format!(
                "migrations/{name}: version {version} is used by another migration"
            ));
        }
    }
    let base_names: BTreeSet<&str> = base.iter().map(String::as_str).collect();
    let Some(newest) = base.iter().filter_map(|name| version(name)).max() else {
        return violations;
    };
    for name in head
        .iter()
        .filter(|name| !base_names.contains(name.as_str()))
    {
        if let Some(version) = version(name)
            && version <= newest
        {
            violations.push(format!(
                "migrations/{name}: a new migration must sort after every migration on the \
                 base (newest {newest}); rename it with a later version"
            ));
        }
    }
    violations
}

/// Whether `name` is an SQL file.
fn is_sql(name: &str) -> bool {
    Path::new(name)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("sql"))
}

/// The `.sql` file names in `migrations/` of the working tree.
fn head(root: &Path) -> Result<Vec<String>> {
    let mut names = Vec::new();
    for entry in fs::read_dir(root.join("migrations")).context("reading migrations/")? {
        let name = entry?.file_name().to_string_lossy().into_owned();
        if is_sql(&name) {
            names.push(name);
        }
    }
    names.sort();
    Ok(names)
}

/// The `.sql` file names in `migrations/` at git `reference`.
fn at(root: &Path, reference: &str) -> Result<Vec<String>> {
    let output = Command::new("git")
        .current_dir(root)
        .args(["ls-tree", "--name-only", reference, "--", "migrations/"])
        .output()
        .context("running git ls-tree")?;
    if !output.status.success() {
        bail!(
            "git ls-tree {reference} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    let mut names: Vec<String> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.strip_prefix("migrations/"))
        .filter(|name| is_sql(name))
        .map(str::to_owned)
        .collect();
    names.sort();
    Ok(names)
}

/// Run the check of the working tree against `base`.
pub fn run(root: &Path, base: &str) -> Result<String> {
    let base_names = at(root, base)?;
    let head_names = head(root)?;
    let violations = check_order(&base_names, &head_names);
    if violations.is_empty() {
        let new = head_names.len().saturating_sub(base_names.len());
        Ok(format!(
            "migrations OK: {new} new after {base}, every one sorts after the base's newest"
        ))
    } else {
        bail!("{}", violations.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|&n| n.to_owned()).collect()
    }

    #[test]
    fn new_migrations_after_the_base_pass() {
        let base = names(&["20261017100000_connectors__zoho_mail.sql"]);
        let head = names(&[
            "20261017100000_connectors__zoho_mail.sql",
            "20261017120000_kernel__search_extensions.sql",
        ]);
        assert!(check_order(&base, &head).is_empty());
    }

    #[test]
    fn a_new_migration_before_or_at_the_base_newest_fails() {
        let base = names(&[
            "20261015090000_conversations__drafts.sql",
            "20261017100000_connectors__zoho_mail.sql",
        ]);
        let head = names(&[
            "20261015090000_conversations__drafts.sql",
            "20261016000000_search__init.sql",
            "20261017100000_connectors__zoho_mail.sql",
            "20261017100000_kernel__search_extensions.sql",
        ]);
        let violations = check_order(&base, &head);
        assert_eq!(violations.len(), 3, "{violations:?}");
        assert!(violations[0].contains("version 20261017100000 is used by another"));
        assert!(violations[1].contains("20261016000000_search__init.sql"));
        assert!(violations[2].contains("20261017100000_kernel__search_extensions.sql"));
    }

    #[test]
    fn an_empty_base_accepts_anything_unique() {
        assert!(check_order(&[], &names(&["1_a__b.sql", "2_a__c.sql"])).is_empty());
    }
}
