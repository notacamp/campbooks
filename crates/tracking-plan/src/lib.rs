//! `campbooks-tracking-plan` — the tracking plan's tooling (ADR 0020,
//! `docs/tracking.md`).
//!
//! The plan is the workbook `tracking/tracking-plan.xlsx`. This crate reads
//! it, checks it, writes it in canonical form, applies YAML patches to it,
//! and generates from it the canonical mirror `tracking/tracking-plan.json`
//! and one `tracking.gen.rs` per owning crate. It depends on no other
//! Campbooks crate, so CI and agents run it without building the server:
//!
//! ```text
//! cargo run -p campbooks-tracking-plan -- check
//! cargo run -p campbooks-tracking-plan -- generate
//! cargo run -p campbooks-tracking-plan -- fmt
//! cargo run -p campbooks-tracking-plan -- apply tracking/patches/0002-things.yaml
//! cargo run -p campbooks-tracking-plan -- init
//! ```
//!
//! `cargo xtask tracking` runs the same commands.

#![forbid(unsafe_code)]

pub mod check;
pub mod generate;
pub mod model;
pub mod patch;
pub mod workbook;

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result, bail};

pub use model::Plan;

/// The workbook, relative to the repository root.
pub const WORKBOOK: &str = "tracking/tracking-plan.xlsx";

/// The operation catalogues Coverage rows of kind `operation` are checked
/// against, relative to the repository root.
pub const OPENAPI_DOCUMENTS: &[&str] = &["openapi/openapi.json", "openapi/admin.json"];

/// What a check found.
#[derive(Debug, Default)]
pub struct Report {
    /// Broken rules: the check fails.
    pub problems: Vec<String>,
    /// Reported only, until campbooks-cloud#332 makes them problems.
    pub warnings: Vec<String>,
}

/// The repository root this crate was built in (`crates/tracking-plan/../..`).
pub fn default_root() -> PathBuf {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .and_then(Path::parent)
        .unwrap_or(manifest)
        .to_path_buf()
}

/// The module crates: directory names under `crates/modules` with a manifest.
pub fn modules(root: &Path) -> Result<BTreeSet<String>> {
    let dir = root.join("crates/modules");
    let mut modules = BTreeSet::new();
    for entry in fs::read_dir(&dir).with_context(|| format!("listing {}", dir.display()))? {
        let entry = entry?;
        if entry.path().join("Cargo.toml").is_file()
            && let Some(name) = entry.file_name().to_str()
        {
            modules.insert(name.to_owned());
        }
    }
    Ok(modules)
}

/// Read the plan, failing with every problem of the workbook's shape.
pub fn read(root: &Path) -> Result<Plan> {
    match workbook::read(&root.join(WORKBOOK))? {
        Ok(plan) => Ok(plan),
        Err(problems) => bail!("{WORKBOOK} is malformed:\n{}", bullets(&problems)),
    }
}

/// Read the plan and require it to pass every rule of the plan itself.
fn read_valid(root: &Path) -> Result<Plan> {
    let plan = read(root)?;
    let problems = check::plan(&plan, &modules(root)?);
    if !problems.is_empty() {
        bail!(
            "the tracking plan breaks {} rule(s):\n{}",
            problems.len(),
            bullets(&problems)
        );
    }
    Ok(plan)
}

fn bullets(lines: &[String]) -> String {
    lines.iter().fold(String::new(), |mut out, line| {
        let _ = writeln!(out, "  - {line}");
        out
    })
}

/// `init`: a new workbook with every sheet, the fixed Actors rows and the
/// README. Refuses to overwrite one.
pub fn init(root: &Path) -> Result<String> {
    let path = root.join(WORKBOOK);
    if path.exists() {
        bail!("{WORKBOOK} exists already; change it with `apply` or by hand");
    }
    workbook::write(&path, &Plan::empty())?;
    Ok(format!("wrote {WORKBOOK}"))
}

/// `fmt`: rewrite the workbook in canonical form.
pub fn fmt(root: &Path) -> Result<String> {
    let plan = read(root)?;
    workbook::write(&root.join(WORKBOOK), &plan)?;
    Ok(format!("formatted {WORKBOOK}"))
}

/// `generate`: write the mirror and every generated Rust file, and delete
/// generated files the plan no longer has.
pub fn generate(root: &Path) -> Result<String> {
    let plan = read_valid(root)?;
    write_generated(root, &plan)
}

fn write_generated(root: &Path, plan: &Plan) -> Result<String> {
    let files = generate::files(plan);
    let mut written = Vec::new();
    for (path, content) in &files {
        let full = root.join(path);
        if fs::read_to_string(&full).ok().as_deref() == Some(content.as_str()) {
            continue;
        }
        if let Some(dir) = full.parent() {
            fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
        }
        fs::write(&full, content).with_context(|| format!("writing {path}"))?;
        written.push(format!("wrote {path}"));
    }
    for path in generated_on_disk(root)? {
        if !files.contains_key(&path) {
            fs::remove_file(root.join(&path)).with_context(|| format!("removing {path}"))?;
            written.push(format!("removed {path}"));
        }
    }
    if written.is_empty() {
        Ok("the generated files are up to date".to_owned())
    } else {
        Ok(written.join("\n"))
    }
}

/// `apply <patch>`: add or update rows, then format the workbook and
/// regenerate. Nothing is written when the patched plan breaks a rule.
pub fn apply(root: &Path, patch_path: &Path) -> Result<String> {
    let text = fs::read_to_string(patch_path)
        .with_context(|| format!("reading {}", patch_path.display()))?;
    let mut plan = read(root)?;
    patch::apply(&mut plan, patch::parse(&text)?)?;
    let problems = check::plan(&plan, &modules(root)?);
    if !problems.is_empty() {
        bail!(
            "after {}, the tracking plan would break {} rule(s); nothing was written:\n{}",
            patch_path.display(),
            problems.len(),
            bullets(&problems)
        );
    }
    workbook::write(&root.join(WORKBOOK), &plan)?;
    let generated = write_generated(root, &plan)?;
    Ok(format!(
        "applied {} to {WORKBOOK}\n{generated}",
        patch_path.display()
    ))
}

/// `check`: every rule. Problems fail; warnings are reported.
pub fn check(root: &Path) -> Result<Report> {
    let mut report = Report::default();
    let plan = match workbook::read(&root.join(WORKBOOK))? {
        Ok(plan) => plan,
        Err(problems) => {
            report.problems = problems;
            return Ok(report);
        }
    };
    report.problems.extend(check::plan(&plan, &modules(root)?));

    let files = generate::files(&plan);
    for (path, content) in &files {
        match fs::read_to_string(root.join(path)) {
            Ok(found) if found == *content => {}
            Ok(_) => report.problems.push(format!(
                "{path} is stale: it is not what {WORKBOOK} generates (run the tool's \
                 `generate`; never edit it by hand)"
            )),
            Err(_) => report
                .problems
                .push(format!("{path} is missing (run the tool's `generate`)")),
        }
        if let Some(problem) = included(root, path) {
            report.problems.push(problem);
        }
    }
    for path in generated_on_disk(root)? {
        if !files.contains_key(&path) {
            report.problems.push(format!(
                "{path} is generated from no part of the plan: run the tool's `generate`, \
                 which removes it"
            ));
        }
    }

    let mut sources = Vec::new();
    rust_sources(root, &root.join("crates"), &mut sources)?;
    report.problems.extend(check::code(&plan, &sources));

    let mut documents = Vec::new();
    for document in OPENAPI_DOCUMENTS {
        let text = fs::read_to_string(root.join(document))
            .with_context(|| format!("reading {document}"))?;
        documents.push(serde_json::from_str(&text).with_context(|| format!("parsing {document}"))?);
    }
    let (problems, unmapped) = check::operations(&plan, &documents);
    report.problems.extend(problems);
    if !unmapped.is_empty() {
        report.warnings.push(format!(
            "{} mutating operation(s) have no Coverage row yet (required once \
             campbooks-cloud#332 lands): {}",
            unmapped.len(),
            unmapped.join(", ")
        ));
    }
    Ok(report)
}

/// A problem when the crate a generated file belongs to does not include
/// it.
fn included(root: &Path, generated: &str) -> Option<String> {
    let (includer, line) = if generated.starts_with("crates/kernel/") {
        (
            "crates/kernel/src/tracking/mod.rs",
            "#[path = \"tracking.gen.rs\"] mod generated;",
        )
    } else if Path::new(generated)
        .extension()
        .is_some_and(|extension| extension == "rs")
    {
        let dir = generated.trim_end_matches("tracking.gen.rs");
        return lib_includes(root, &format!("{dir}lib.rs"));
    } else {
        return None;
    };
    let text = fs::read_to_string(root.join(includer)).unwrap_or_default();
    (!text.contains("\"tracking.gen.rs\""))
        .then(|| format!("{includer} does not include the generated file: add `{line}`"))
}

fn lib_includes(root: &Path, lib: &str) -> Option<String> {
    let text = fs::read_to_string(root.join(lib)).unwrap_or_default();
    (!text.contains("\"tracking.gen.rs\"")).then(|| {
        format!(
            "{lib} does not include its generated events: add \
             `#[path = \"tracking.gen.rs\"] pub mod tracking;`"
        )
    })
}

/// Every `tracking.gen.rs` under `crates/`, relative to the root.
fn generated_on_disk(root: &Path) -> Result<Vec<String>> {
    let mut sources = Vec::new();
    rust_sources(root, &root.join("crates"), &mut sources)?;
    Ok(sources
        .into_iter()
        .map(|(path, _)| path)
        .filter(|path| path.ends_with("/tracking.gen.rs"))
        .collect())
}

/// Every `.rs` file under `dir` (skipping `target`), with its path relative
/// to `root` (`/`-separated) and its text.
fn rust_sources(root: &Path, dir: &Path, out: &mut Vec<check::Source>) -> Result<()> {
    let mut entries: Vec<_> = fs::read_dir(dir)
        .with_context(|| format!("listing {}", dir.display()))?
        .collect::<Result<_, _>>()?;
    entries.sort_by_key(fs::DirEntry::path);
    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            if entry.file_name() != "target" && entry.file_name() != "node_modules" {
                rust_sources(root, &path, out)?;
            }
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            let text =
                fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
            let relative = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .components()
                .map(|c| c.as_os_str().to_string_lossy())
                .collect::<Vec<_>>()
                .join("/");
            out.push((relative, text));
        }
    }
    Ok(())
}
