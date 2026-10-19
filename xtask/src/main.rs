//! `cargo xtask` — repository automation.
//!
//! - `cargo xtask arch` enforces the architecture rules in `AGENTS.md`.
//! - `cargo xtask openapi [--check]` regenerates (or verifies) `openapi/openapi.json`
//!   and `openapi/admin.json`.
//! - `cargo xtask events [--check]` regenerates (or verifies) `openapi/events.json`.
//! - `cargo xtask eval decide` measures `decide` strategies on a labelled eval set.
//! - `cargo xtask eval actions` scores the action catalogue on an owner's labels.
//! - `cargo xtask migrations --base <ref>` checks that new migrations sort after
//!   every migration on the base branch.
//! - `cargo xtask i18n` checks that every server message catalogue is at parity.
//! - `cargo xtask new-module <name>` and `cargo xtask new-feature <module> <feature>`
//!   scaffold a module or a feature slice; `cargo xtask scaffold-check` proves
//!   their output passes the gates.

#![forbid(unsafe_code)]

mod access;
mod arch;
mod eval;
mod events;
mod i18n;
mod migrations;
mod openapi;
mod payloads;
mod scaffold;

use std::{
    collections::BTreeSet,
    fmt::Write as _,
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
    /// they touch, versions, scope and background-work names, that every
    /// OpenAPI operation declares its security and is a complete catalogue
    /// entry (ADR 0013), that every consumed event is published, and the
    /// instance-administration rules of ADR 0015.
    Arch,
    /// Write openapi/openapi.json and openapi/admin.json from the server's
    /// routes.
    Openapi {
        /// Fail if the committed document is stale instead of writing it.
        #[arg(long)]
        check: bool,
    },
    /// Measure AI strategies on a labelled eval set.
    Eval {
        #[command(subcommand)]
        what: EvalCommand,
    },
    /// Write openapi/events.json from the modules' published events.
    Events {
        /// Fail if the committed catalogue is stale instead of writing it.
        #[arg(long)]
        check: bool,
    },
    /// Check that the migrations added since git `<base>` sort after every
    /// migration on it, and that no version is used twice (CI runs it on
    /// pull requests against their base branch).
    Migrations {
        /// The base to compare with, e.g. `origin/next`.
        #[arg(long)]
        base: String,
    },
    /// Check that every server message catalogue (`crates/**/i18n/`) has
    /// `en`, `pt-PT`, `es-ES` and `fr` with the same messages, plural forms
    /// and placeholders, and no empty values.
    I18n,
    /// Scaffold a domain module: crate (scopes, events, an `overview` read
    /// slice), schema migration, OpenSpec change, client module with i18n,
    /// registered in the workspace, the server and the client shell.
    NewModule {
        /// The module's `snake_case` name; also its schema and OAuth scope prefix.
        name: String,
        /// The tracking issue URL, recorded in the OpenSpec change.
        #[arg(long)]
        issue: Option<String>,
    },
    /// Scaffold a feature slice in a module: a scoped `GET` handler with its
    /// OpenAPI path and tests, mounted on the module's router.
    NewFeature {
        /// The module (directory under crates/modules).
        module: String,
        /// The feature's `snake_case` name.
        feature: String,
    },
    /// Generate a module and a feature in a temporary copy of the repository
    /// and run the gates on them.
    ScaffoldCheck {
        /// Also run the client gates and check that OpenSpec rejects the
        /// unfinished change (needs `pnpm install` in clients/ and npx).
        #[arg(long)]
        full: bool,
    },
}

#[derive(Debug, Subcommand)]
enum EvalCommand {
    /// Run `decide` strategies (rules, k-NN, models, cascades) over an eval
    /// set and report accuracy, coverage and precision at the auto-act
    /// threshold, calibration and latency per question and locale.
    Decide(eval::DecideArgs),
    /// Score the action catalogue's ratings against an owner's labelled
    /// export at one or more floors: precision and recall per action, and
    /// how often pairs of actions are done together (campbooks-cloud#166).
    Actions(eval::ActionsArgs),
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let root = workspace_root();
    let result = match cli.command {
        Command::Arch => run_arch(&root),
        Command::Openapi { check } => openapi::run(&root, check),
        Command::Events { check } => events::run(&root, check),
        Command::Eval {
            what: EvalCommand::Decide(args),
        } => eval::run_decide(&root, &args),
        Command::Eval {
            what: EvalCommand::Actions(args),
        } => eval::run_actions(&args),
        Command::Migrations { base } => migrations::run(&root, &base),
        Command::I18n => i18n::run(&root),
        Command::NewModule { name, issue } => run_new_module(&root, &name, issue),
        Command::NewFeature { module, feature } => run_new_feature(&root, &module, &feature),
        Command::ScaffoldCheck { full } => scaffold::check::run(&root, full),
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

fn run_new_module(root: &Path, name: &str, issue: Option<String>) -> Result<String> {
    let options = scaffold::ModuleOptions {
        issue,
        now: chrono::Utc::now(),
    };
    let plan = scaffold::new_module(root, name, &options)?;
    let warnings = plan.apply(root)?;
    let kebab = name.replace('_', "-");
    Ok(format!(
        "module `{name}` scaffolded:\n{}{}\nNext:\n  \
         1. cargo xtask openapi && pnpm --dir clients api:generate\n  \
         2. Fill in every TODO(new-module) (`git grep -n 'TODO(new-module)'`). Until the spec \
         delta states real requirements, `openspec validate add-{kebab}-module --strict` fails, \
         so the change cannot merge unfinished.\n  \
         3. Add use cases with `cargo xtask new-feature {name} <feature>`.\n  \
         4. Run the gates (AGENTS.md -> Code quality) and commit Cargo.lock with the rest.",
        plan.report(),
        format_warnings(&warnings),
    ))
}

fn run_new_feature(root: &Path, module: &str, feature: &str) -> Result<String> {
    let plan = scaffold::new_feature(root, module, feature)?;
    let warnings = plan.apply(root)?;
    Ok(format!(
        "feature `{module}::{feature}` scaffolded:\n{}{}\nNext:\n  \
         1. Replace the placeholder response and every TODO(new-feature); describe the \
         behaviour in the module's OpenSpec change.\n  \
         2. cargo xtask openapi && pnpm --dir clients api:generate\n  \
         3. Run the gates (AGENTS.md -> Code quality).",
        plan.report(),
        format_warnings(&warnings),
    ))
}

fn format_warnings(warnings: &[String]) -> String {
    warnings.iter().fold(String::new(), |mut out, w| {
        let _ = writeln!(out, "  warning: {w}");
        out
    })
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
    violations.extend(arch::check_reserved_owner(
        &modules,
        campbooks_server::changes::OWNER,
    ));
    violations.extend(arch::check_migration_schemas(&migration_sql, &modules));
    let registry = campbooks_server::registry();
    let background = registry
        .as_ref()
        .err()
        .map(|e| e.0.clone())
        .unwrap_or_default();
    violations.extend(arch::check_background(&background));
    if let Ok(registry) = &registry {
        violations.extend(event_violations(registry));
    }
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
    let schemes = arch::Schemes {
        bearer: campbooks_server::SECURITY_SCHEME,
        session: campbooks_server::bff::SESSION_SCHEME,
    };
    violations.extend(arch::check_operation_security(&spec, schemes, &catalogue));
    let owners = operation_owners()?;
    violations.extend(arch::check_operations(&spec, &owners));
    violations.extend(arch::check_answer_field_names(
        &spec,
        arch::ANSWER_FIELDS_ALLOWED,
    ));
    violations.extend(check_payloads(root)?);
    violations.extend(check_admin(root, &spec, &owners)?);
    violations.extend(check_access(root, &spec, &arch_file, &migration_sql)?);

    if violations.is_empty() {
        let modules: Vec<_> = modules.iter().map(String::as_str).collect();
        return Ok(format!(
            "architecture OK: {} crates, {} module(s) [{}], {} migration(s), {} scope(s), \
             every operation declares its security and catalogue metadata",
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

/// Personal data stays out of event payloads, job payloads and job errors
/// (campbooks-cloud#65, `payloads`).
fn check_payloads(root: &Path) -> Result<Vec<String>> {
    let events = serde_json::to_value(
        campbooks_server::events_catalogue().context("building the events catalogue")?,
    )
    .context("serializing the events catalogue")?;
    let mut violations = payloads::check_event_payloads(&events, payloads::EVENT_FIELDS_ALLOWED);
    let mut sources = Vec::new();
    rust_sources(root, &root.join("crates"), &mut sources)?;
    violations.extend(payloads::check_job_payloads(
        &payloads::job_payloads(&sources),
        payloads::JOB_PAYLOAD_FIELDS,
    ));
    violations.extend(payloads::check_job_errors(
        &payloads::job_error_values(&sources),
        payloads::JOB_ERROR_VALUES,
    ));
    Ok(violations)
}

/// Every consumed event is published, at the version read.
fn event_violations(registry: &campbooks_kernel::jobs::Registry) -> Vec<String> {
    let published: Vec<arch::EventDecl> = registry
        .published()
        .iter()
        .map(|e| arch::EventDecl {
            by: e.module.to_owned(),
            name: e.name.to_owned(),
            version: e.version,
        })
        .collect();
    let consumed: Vec<arch::EventDecl> = registry
        .consumers()
        .iter()
        .map(|c| arch::EventDecl {
            by: c.name.to_owned(),
            name: c.event.to_owned(),
            version: c.event_version,
        })
        .collect();
    arch::check_events(&published, &consumed)
}

/// The rules of ADR 0017: every operation declares a catalogued resource
/// and action, its handler asks the central check, module code queries
/// workspace tables only in a workspace transaction, and every published
/// event has a consumer or a stated reason.
fn check_access(
    root: &Path,
    spec: &serde_json::Value,
    arch_file: &arch::ArchitectureFile,
    migrations: &[(String, String)],
) -> Result<Vec<String>> {
    let registry = campbooks_server::registry().map_err(|e| anyhow::anyhow!("{e}"))?;
    let catalogue: std::collections::BTreeMap<String, access::Resource> = registry
        .resources()
        .iter()
        .map(|declared| {
            let tenancy = serde_json::to_value(declared.tenancy)
                .ok()
                .and_then(|v| v.as_str().map(ToOwned::to_owned))
                .unwrap_or_default();
            let actions = declared
                .actions()
                .iter()
                .map(|a| a.as_str().to_owned())
                .collect();
            (
                declared.name.to_owned(),
                access::Resource { tenancy, actions },
            )
        })
        .collect();
    let mut sources = Vec::new();
    rust_sources(root, &root.join("crates"), &mut sources)?;
    let published: BTreeSet<String> = registry
        .published()
        .iter()
        .map(|e| e.name.to_owned())
        .collect();
    let consumed: BTreeSet<String> = registry
        .consumers()
        .iter()
        .map(|c| c.event.to_owned())
        .collect();
    let tables = access::workspace_tables(migrations);
    let mut violations = access::check_declarations(spec, &catalogue);
    violations.extend(access::check_authorize_calls(
        spec,
        &catalogue,
        &sources,
        &arch_file.access,
    ));
    violations.extend(access::check_workspace_queries(
        &sources,
        &tables,
        &arch_file.access,
    ));
    violations.extend(access::check_kernel_scope(&sources, &arch_file.access));
    violations.extend(access::check_consumers(
        &published,
        &consumed,
        &arch_file.access,
    ));
    Ok(violations)
}

/// The rules of ADR 0015: the admin module serves nothing, the admin
/// document's operations are well-formed and stay out of the catalogue, and
/// only the admin module, the server's CLI and the kernel name what they own.
fn check_admin(
    root: &Path,
    spec: &serde_json::Value,
    owners: &arch::Owners,
) -> Result<Vec<String>> {
    let mut violations = Vec::new();
    for module in campbooks_server::modules() {
        if module.name == arch::ADMIN_MODULE {
            let routes = module.router.into_openapi().paths.paths.len();
            let admin_routes = module
                .admin_router
                .map(|router| router.into_openapi().paths.paths.len());
            violations.extend(arch::check_admin_module(routes, admin_routes));
        }
    }
    let admin_owners = admin_operation_owners()?;
    violations.extend(arch::check_no_admin_owner(owners, &admin_owners, spec));
    let admin_spec = serde_json::to_value(campbooks_server::admin_openapi())
        .context("serializing the admin OpenAPI document")?;
    let permissions: BTreeSet<String> = campbooks_kernel::admin::Permission::ALL
        .iter()
        .map(|p| p.as_str().to_owned())
        .collect();
    let catalogue_ids: BTreeSet<String> = spec["paths"]
        .as_object()
        .into_iter()
        .flat_map(|paths| paths.values())
        .filter_map(serde_json::Value::as_object)
        .flat_map(|item| item.values())
        .filter_map(|op| op.get("operationId").and_then(serde_json::Value::as_str))
        .map(ToOwned::to_owned)
        .collect();
    violations.extend(arch::check_admin_operations(
        &admin_spec,
        &admin_owners,
        &permissions,
        &catalogue_ids,
    ));
    let mut sources = Vec::new();
    rust_sources(root, &root.join("crates"), &mut sources)?;
    violations.extend(arch::check_admin_sources(&sources));
    Ok(violations)
}

/// `(METHOD, path)` → the module whose admin router declares the operation.
fn admin_operation_owners() -> Result<arch::Owners> {
    let mut owners = arch::Owners::new();
    for module in campbooks_server::modules() {
        let Some(router) = module.admin_router else {
            continue;
        };
        let spec = serde_json::to_value(router.into_openapi())
            .with_context(|| format!("serializing `{}`'s admin OpenAPI", module.name))?;
        for (path, item) in spec["paths"].as_object().cloned().unwrap_or_default() {
            for method in arch::OPERATION_METHODS {
                if item.get(method).is_some() {
                    owners.insert(
                        (method.to_uppercase(), path.clone()),
                        module.name.to_owned(),
                    );
                }
            }
        }
    }
    Ok(owners)
}

/// Every `.rs` file under `dir`, as (path relative to `root`, text), in path
/// order: the file system's own order differs between machines and runs, and
/// no check may depend on it.
fn rust_sources(root: &Path, dir: &Path, out: &mut Vec<(String, String)>) -> Result<()> {
    let mut paths = Vec::new();
    for entry in fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))? {
        paths.push(entry?.path());
    }
    paths.sort();
    for path in paths {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        if name.starts_with('.') || name == "target" || name == "node_modules" {
            continue;
        }
        if path.is_dir() {
            rust_sources(root, &path, out)?;
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            let text =
                fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
            let rel = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            out.push((rel, text));
        }
    }
    Ok(())
}

/// `(METHOD, path)` → the module whose router declares the operation,
/// `bff` for the web BFF's own `/bff/*` operations, or `changes` for the
/// change feed (no module may take either name).
fn operation_owners() -> Result<arch::Owners> {
    let mut owners = arch::Owners::new();
    let documents = campbooks_server::modules()
        .into_iter()
        .map(|module| (module.name, module.router.into_openapi()))
        .chain([
            (arch::BFF_SCHEMA, campbooks_server::bff::openapi()),
            // The issuer's protocol endpoints belong to the identity module.
            ("identity", campbooks_server::identity_protocol_openapi()),
            (
                campbooks_server::changes::OWNER,
                campbooks_server::changes::router().into_openapi(),
            ),
        ]);
    for (name, document) in documents {
        let spec = serde_json::to_value(document)
            .with_context(|| format!("serializing `{name}`'s OpenAPI"))?;
        let paths = spec["paths"].as_object().cloned().unwrap_or_default();
        for (path, item) in paths {
            for method in arch::OPERATION_METHODS {
                if item.get(method).is_some() {
                    owners.insert((method.to_uppercase(), path.clone()), name.to_owned());
                }
            }
        }
    }
    Ok(owners)
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
