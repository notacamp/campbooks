//! `cargo xtask scaffold-check [--full]`: prove the generators' output
//! compiles and passes the gates, without touching the working tree.
//!
//! It copies the repository (tracked and untracked, non-ignored files) to a
//! temporary directory, runs `new-module` and `new-feature` there, and runs
//! the Rust gates on the result: fmt, OpenAPI regeneration, `arch`, clippy,
//! the new module's tests and (when installed) `cargo machete`. With
//! `--full` it also installs the client dependencies in the copy, runs the
//! client gates against the regenerated API types, and checks that OpenSpec
//! rejects the unfinished change.
//!
//! The copy builds into this checkout's target directory, so dependencies
//! are not compiled twice. Cargo re-links a checkout's own binaries
//! (`target/debug/xtask`, the server) whenever it runs them, so the copy's
//! builds never stand in for this checkout's. The copy lives at a path
//! derived from this checkout's, so repeated runs reuse its artifacts
//! instead of piling up new ones.

use std::{
    ffi::OsStr,
    fs,
    hash::{DefaultHasher, Hash as _, Hasher as _},
    io::Write as _,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context as _, Result, bail};
use chrono::Utc;

use super::{ModuleOptions, new_feature, new_module};

/// The module and feature the check generates.
const PROBE_MODULE: &str = "scaffold_probe";
const PROBE_FEATURE: &str = "list_items";

/// Version of the OpenSpec CLI, as in CI.
const OPENSPEC: &str = "@fission-ai/openspec@1.13.2";

/// Client gates run by `--full`, in CI's order (`api:check` is replaced by
/// regenerating the types, since the copy is not a git checkout).
const CLIENT_GATES: [&str; 8] = [
    "api:generate",
    "lint",
    "format:check",
    "typecheck",
    "deps:check",
    "deadcode",
    "i18n:check",
    "test",
];

/// Run the check. Keeps the copy for inspection when a step fails.
pub fn run(root: &Path, full: bool) -> Result<String> {
    let copy = std::env::temp_dir().join(format!("campbooks-scaffold-check-{:016x}", {
        let mut hasher = DefaultHasher::new();
        root.hash(&mut hasher);
        hasher.finish()
    }));
    if copy.exists() {
        fs::remove_dir_all(&copy).with_context(|| format!("clearing {}", copy.display()))?;
    }
    copy_repository(root, &copy)?;
    let target =
        std::env::var_os("CARGO_TARGET_DIR").map_or_else(|| root.join("target"), PathBuf::from);

    let result = generate_and_check(&copy, &target, full);
    match result {
        Ok(steps) => {
            fs::remove_dir_all(&copy).with_context(|| format!("removing {}", copy.display()))?;
            Ok(format!(
                "scaffold OK: `new-module {PROBE_MODULE}` + `new-feature {PROBE_MODULE} \
                 {PROBE_FEATURE}` pass {steps} gate(s)"
            ))
        }
        Err(error) => {
            Err(error.context(format!("the generated copy is kept at {}", copy.display())))
        }
    }
}

fn generate_and_check(copy: &Path, target: &Path, full: bool) -> Result<usize> {
    if full {
        // Before generating, so the generator's prettier pass runs too. From
        // pnpm's store, so it is quick and offline when `clients/` is installed.
        let mut install = Command::new("pnpm");
        install
            .args([
                "--dir",
                "clients",
                "install",
                "--frozen-lockfile",
                "--prefer-offline",
            ])
            .env("CI", "true")
            .current_dir(copy);
        run_step(
            "pnpm install --frozen-lockfile --prefer-offline",
            &mut install,
        )?;
    }
    let options = ModuleOptions {
        issue: None,
        now: Utc::now(),
    };
    let mut warnings = new_module(copy, PROBE_MODULE, &options)?.apply(copy)?;
    // The feature is planned once the module exists on disk.
    warnings.extend(new_feature(copy, PROBE_MODULE, PROBE_FEATURE)?.apply(copy)?);
    // Without `--full` the copy has no client dependencies to format with;
    // a rustfmt failure is caught by the fmt gate either way.
    if full {
        warn_all(&warnings);
    }

    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let probe_crate = format!("campbooks-{}", super::names::kebab(PROBE_MODULE));
    let mut test = vec!["test", "--package", probe_crate.as_str()];
    if std::env::var_os("DATABASE_URL").is_none() {
        note("DATABASE_URL is unset: skipping the generated schema test");
        test.extend(["--", "--skip", "migrations_create_the"]);
    }
    let mut steps: Vec<(String, Command)> = vec![
        cargo_step(&cargo, copy, target, &["fmt", "--all", "--check"]),
        cargo_step(
            &cargo,
            copy,
            target,
            &["run", "--quiet", "--package", "xtask", "--", "openapi"],
        ),
        cargo_step(
            &cargo,
            copy,
            target,
            &["run", "--quiet", "--package", "xtask", "--", "arch"],
        ),
        cargo_step(
            &cargo,
            copy,
            target,
            &[
                "clippy",
                "--workspace",
                "--all-targets",
                "--",
                "-D",
                "warnings",
            ],
        ),
        cargo_step(&cargo, copy, target, &test),
    ];
    // Called directly: as `cargo machete` under `cargo run`, it reads its
    // arguments as paths.
    if installed("cargo-machete") {
        let mut machete = Command::new("cargo-machete");
        machete.current_dir(copy);
        steps.push(("cargo machete".to_owned(), machete));
    } else {
        note("cargo-machete is not installed: skipping the unused-dependency check");
    }
    let mut count = 0;
    for (label, command) in &mut steps {
        run_step(label, command)?;
        count += 1;
    }
    if full {
        count += check_clients(copy)?;
    }
    Ok(count)
}

/// Client gates and the OpenSpec refusal, against the copy.
fn check_clients(copy: &Path) -> Result<usize> {
    let mut count = 0;
    for gate in CLIENT_GATES {
        let mut command = Command::new("pnpm");
        command.args(["--dir", "clients", gate]).current_dir(copy);
        run_step(&format!("pnpm {gate}"), &mut command)?;
        count += 1;
    }

    let change = format!("add-{}-module", super::names::kebab(PROBE_MODULE));
    let mut validate = Command::new("npx");
    validate
        .args([
            "--yes",
            OPENSPEC,
            "validate",
            &change,
            "--strict",
            "--no-interactive",
        ])
        .env("OPENSPEC_TELEMETRY", "0")
        .current_dir(copy);
    let status = validate
        .output()
        .with_context(|| format!("running `openspec validate {change}`"))?
        .status;
    if status.success() {
        bail!(
            "`openspec validate {change} --strict` accepted the unfinished change; its \
             TODO(new-module) requirement must keep it from merging"
        );
    }
    note(&format!(
        "openspec rejects the unfinished `{change}`, as it should"
    ));
    Ok(count + 1)
}

fn cargo_step(cargo: &OsStr, copy: &Path, target: &Path, args: &[&str]) -> (String, Command) {
    let mut command = Command::new(cargo);
    command
        .args(args)
        .current_dir(copy)
        .env("CARGO_TARGET_DIR", target)
        // Queries compile against the committed `.sqlx` cache, as in CI; the
        // generated code adds none. `DATABASE_URL` is only for test runs.
        .env("SQLX_OFFLINE", "true");
    (format!("cargo {}", args.join(" ")), command)
}

fn installed(program: &str) -> bool {
    Command::new(program)
        .arg("--version")
        .output()
        .is_ok_and(|out| out.status.success())
}

fn run_step(label: &str, command: &mut Command) -> Result<()> {
    note(&format!("running `{label}`"));
    let status = command
        .status()
        .with_context(|| format!("starting `{label}`"))?;
    if !status.success() {
        bail!("`{label}` failed on the generated code ({status})");
    }
    Ok(())
}

/// Copy every tracked and untracked, non-ignored file of `root` to `copy`.
fn copy_repository(root: &Path, copy: &Path) -> Result<()> {
    let output = Command::new("git")
        .args([
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
        ])
        .current_dir(root)
        .output()
        .context("running `git ls-files`")?;
    if !output.status.success() {
        bail!(
            "`git ls-files` failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    for name in output.stdout.split(|b| *b == 0).filter(|n| !n.is_empty()) {
        let name = std::str::from_utf8(name).context("a file name is not UTF-8")?;
        let from = root.join(name);
        // Deleted but not yet staged, or a directory (submodule): skip.
        if !from.is_file() {
            continue;
        }
        let to = copy.join(name);
        if let Some(dir) = to.parent() {
            fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
        }
        fs::copy(&from, &to).with_context(|| format!("copying {name}"))?;
    }
    Ok(())
}

fn warn_all(warnings: &[String]) {
    for warning in warnings {
        note(&format!("warning: {warning}"));
    }
}

fn note(message: &str) {
    let _ = writeln!(std::io::stderr(), "scaffold-check: {message}");
}
