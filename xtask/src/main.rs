//! `cargo xtask` — repository automation.
//!
//! - `cargo xtask arch` enforces the architecture rules in `AGENTS.md`.
//! - `cargo xtask openapi [--check]` regenerates (or verifies) `openapi/openapi.json`.

#![forbid(unsafe_code)]

mod arch;
mod openapi;

use std::{
    collections::BTreeSet,
    fs,
    io::Write as _,
    path::{Path, PathBuf},
    process::ExitCode,
};

use anyhow::{Context as _, Result};
use cargo_metadata::MetadataCommand;
use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "cargo xtask", about = "Campbooks repository automation.")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Check crate dependency directions, migration names and the schemas
    /// they touch, versions, scope and background-work names, and that every
    /// OpenAPI operation declares its security.
    Arch,
    /// Write openapi/openapi.json from the server's routes.
    Openapi {
        /// Fail if the committed document is stale instead of writing it.
        #[arg(long)]
        check: bool,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let root = workspace_root();
    let result = match cli.command {
        Command::Arch => run_arch(&root),
        Command::Openapi { check } => openapi::run(&root, check),
    };
    match result {
        Ok(message) => {
            let _ = writeln!(std::io::stdout(), "{message}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            let _ = writeln!(std::io::stderr(), "error: {e:#}");
            ExitCode::FAILURE
        }
    }
}

fn workspace_root() -> PathBuf {
    let xtask_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    xtask_dir.parent().unwrap_or(xtask_dir).to_path_buf()
}

fn run_arch(root: &Path) -> Result<String> {
    let crates = load_crates(root)?;
    let modules = arch::module_names(&crates);

    let arch_file: arch::ArchitectureFile = toml::from_str(
        &fs::read_to_string(root.join("architecture.toml")).context("reading architecture.toml")?,
    )
    .context("parsing architecture.toml")?;

    let migrations = migration_file_names(&root.join("migrations"))?;
    let migration_sql = migrations
        .iter()
        .filter(|name| {
            Path::new(name)
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("sql"))
        })
        .map(|name| {
            let path = root.join("migrations").join(name);
            fs::read_to_string(&path)
                .with_context(|| format!("reading {}", path.display()))
                .map(|sql| (name.clone(), sql))
        })
        .collect::<Result<Vec<_>>>()?;
    let version = fs::read_to_string(root.join("VERSION")).context("reading VERSION")?;

    let mut violations = Vec::new();
    violations.extend(arch::check_allowlist(&arch_file.allow, &modules));
    violations.extend(arch::check_dependencies(&crates, &arch_file.allow));
    violations.extend(arch::check_migrations(&migrations, &modules));
    violations.extend(arch::check_migration_schemas(&migration_sql, &modules));
    let background = campbooks_server::registry()
        .err()
        .map(|e| e.0)
        .unwrap_or_default();
    violations.extend(arch::check_background(&background));
    violations.extend(arch::check_versions(&version, &crates));

    let scopes: Vec<arch::DeclaredScope> = campbooks_server::modules()
        .iter()
        .flat_map(|module| {
            module.scopes.iter().map(|scope| arch::DeclaredScope {
                module: module.name.to_owned(),
                name: scope.name.to_owned(),
                description: scope.description.to_owned(),
            })
        })
        .collect();
    violations.extend(arch::check_scopes(&scopes));
    let catalogue: BTreeSet<String> = scopes.iter().map(|s| s.name.clone()).collect();
    let spec = serde_json::to_value(campbooks_server::openapi())
        .context("serializing the OpenAPI document")?;
    let operations =
        arch::check_operation_security(&spec, campbooks_server::SECURITY_SCHEME, &catalogue);
    violations.extend(operations);

    if violations.is_empty() {
        let modules: Vec<_> = modules.iter().map(String::as_str).collect();
        return Ok(format!(
            "architecture OK: {} crates, {} module(s) [{}], {} migration(s), {} scope(s), \
             every operation declares its security",
            crates.len(),
            modules.len(),
            modules.join(", "),
            migrations.len(),
            scopes.len()
        ));
    }
    let list: Vec<String> = violations.iter().map(|v| format!("  - {v}")).collect();
    anyhow::bail!(
        "{} architecture violation(s) (see AGENTS.md → Architecture rules):\n{}",
        violations.len(),
        list.join("\n")
    )
}

fn load_crates(root: &Path) -> Result<Vec<arch::Crate>> {
    let metadata = MetadataCommand::new()
        .manifest_path(root.join("Cargo.toml"))
        .no_deps()
        .exec()
        .context("running cargo metadata")?;
    let members: BTreeSet<&str> = metadata
        .workspace_packages()
        .iter()
        .map(|p| p.name.as_str())
        .collect();

    metadata
        .workspace_packages()
        .iter()
        .map(|package| {
            let dir = package
                .manifest_path
                .parent()
                .context("manifest has a parent directory")?;
            let rel = dir
                .strip_prefix(&metadata.workspace_root)
                .with_context(|| format!("{dir} is outside the workspace"))?;
            Ok(arch::Crate {
                name: package.name.to_string(),
                path: rel.as_str().replace('\\', "/"),
                version: package.version.to_string(),
                deps: package
                    .dependencies
                    .iter()
                    .map(|d| d.name.clone())
                    .filter(|name| members.contains(name.as_str()))
                    .collect(),
            })
        })
        .collect()
}

fn migration_file_names(dir: &Path) -> Result<Vec<String>> {
    let mut names = Vec::new();
    for entry in fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))? {
        let name = entry?.file_name().to_string_lossy().into_owned();
        if !name.starts_with('.') {
            names.push(name);
        }
    }
    names.sort();
    Ok(names)
}
