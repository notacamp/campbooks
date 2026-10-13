//! `cargo xtask new-module` and `cargo xtask new-feature`: scaffold a module
//! or a feature slice in the shape of the reference module (`system`).
//!
//! Templates are plain files under `xtask/templates/`, compiled in with
//! `include_str!`, with `{{placeholder}}` substitution and nothing else. A
//! generator first computes every file it creates and every edit to an
//! existing file (a [`Plan`]); only when all of that succeeded does it write,
//! so a refused or failed run leaves the tree untouched. `cargo xtask
//! scaffold-check` proves the output compiles and passes the gates.
//
// TODO(campbooks-cloud#50): module declarations (the action registry) add a
// `declarations` part to every module. When they land: add a
// `templates/module/rust/declarations.rs.tmpl`, list it in `MODULE_TEMPLATES`,
// declare `pub mod declarations;` in `lib.rs.tmpl` and set the new `Module`
// field there; `new-feature` then registers each feature's action next to its
// route (an edit like `edits::add_route`).

pub mod check;
mod edits;
mod names;

use std::{
    fmt::Write as _,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context as _, Result, bail};
use chrono::{DateTime, NaiveDateTime, TimeDelta, Utc};

use self::names::Kind;

/// Placeholder written where the tracking issue URL belongs when
/// `--issue` is not given.
const NO_ISSUE: &str = "TODO(new-module)";

/// The example read slice every new module starts with.
const FIRST_FEATURE: &str = "overview";

/// Migration timestamps: `YYYYMMDDHHMMSS`, UTC.
const TIMESTAMP_FORMAT: &str = "%Y%m%d%H%M%S";

/// A file the scaffold creates, as a path template and a body template.
struct Template {
    path: &'static str,
    body: &'static str,
}

macro_rules! template {
    ($path:literal, $file:literal) => {
        Template {
            path: $path,
            body: include_str!(concat!("../../templates/", $file)),
        }
    };
}

/// Every file `new-module` creates, besides the first feature.
const MODULE_TEMPLATES: &[Template] = &[
    template!(
        "crates/modules/{{name}}/Cargo.toml",
        "module/rust/Cargo.toml.tmpl"
    ),
    template!(
        "crates/modules/{{name}}/src/lib.rs",
        "module/rust/lib.rs.tmpl"
    ),
    template!(
        "crates/modules/{{name}}/src/scopes.rs",
        "module/rust/scopes.rs.tmpl"
    ),
    template!(
        "crates/modules/{{name}}/src/events.rs",
        "module/rust/events.rs.tmpl"
    ),
    template!(
        "crates/modules/{{name}}/src/features/mod.rs",
        "module/rust/features.rs.tmpl"
    ),
    template!(
        "migrations/{{timestamp}}_{{name}}__init.sql",
        "module/migration.sql.tmpl"
    ),
    template!(
        "openspec/changes/add-{{kebab}}-module/.openspec.yaml",
        "module/openspec/openspec.yaml.tmpl"
    ),
    template!(
        "openspec/changes/add-{{kebab}}-module/proposal.md",
        "module/openspec/proposal.md.tmpl"
    ),
    template!(
        "openspec/changes/add-{{kebab}}-module/design.md",
        "module/openspec/design.md.tmpl"
    ),
    template!(
        "openspec/changes/add-{{kebab}}-module/tasks.md",
        "module/openspec/tasks.md.tmpl"
    ),
    template!(
        "openspec/changes/add-{{kebab}}-module/specs/{{name}}/spec.md",
        "module/openspec/spec.md.tmpl"
    ),
    template!(
        "clients/app/src/modules/{{kebab}}/index.ts",
        "module/client/index.ts.tmpl"
    ),
    template!(
        "clients/app/src/modules/{{kebab}}/routes.tsx",
        "module/client/routes.tsx.tmpl"
    ),
    template!(
        "clients/app/src/modules/{{kebab}}/routes.test.tsx",
        "module/client/routes.test.tsx.tmpl"
    ),
    template!(
        "clients/app/src/modules/{{kebab}}/components/{{pascal}}Home/{{pascal}}Home.tsx",
        "module/client/Home.tsx.tmpl"
    ),
    template!(
        "clients/app/src/modules/{{kebab}}/i18n/en.json",
        "module/client/i18n/en.json.tmpl"
    ),
    template!(
        "clients/app/src/modules/{{kebab}}/i18n/pt-PT.json",
        "module/client/i18n/pt-PT.json.tmpl"
    ),
    template!(
        "clients/app/src/modules/{{kebab}}/i18n/es-ES.json",
        "module/client/i18n/es-ES.json.tmpl"
    ),
    template!(
        "clients/app/src/modules/{{kebab}}/i18n/fr.json",
        "module/client/i18n/fr.json.tmpl"
    ),
];

/// The file `new-feature` creates.
const FEATURE_TEMPLATE: Template = template!(
    "crates/modules/{{name}}/src/features/{{feature}}/mod.rs",
    "feature/mod.rs.tmpl"
);

/// Files outside the new module that `new-module` edits to register it.
const REGISTRATION_FILES: [&str; 5] = [
    WORKSPACE_MANIFEST,
    SERVER_MANIFEST,
    SERVER_LIB,
    CLIENT_MODULES,
    CLIENT_I18N_TYPES,
];
const WORKSPACE_MANIFEST: &str = "Cargo.toml";
const SERVER_MANIFEST: &str = "crates/server/Cargo.toml";
const SERVER_LIB: &str = "crates/server/src/lib.rs";
const CLIENT_MODULES: &str = "clients/app/src/app/modules.ts";
const CLIENT_I18N_TYPES: &str = "clients/app/src/app/i18next.d.ts";

/// Placeholder values, as `(name, value)`.
type Vars = Vec<(&'static str, String)>;

/// What a generator will write, computed before anything is written.
#[derive(Debug, Default)]
pub struct Plan {
    /// New files, relative to the workspace root.
    pub create: Vec<(PathBuf, String)>,
    /// Existing files with their new content, relative to the workspace root.
    pub update: Vec<(PathBuf, String)>,
}

impl Plan {
    /// Write every file, then format the Rust and client sources it touched.
    /// Returns warnings about formatting that could not run.
    pub fn apply(&self, root: &Path) -> Result<Vec<String>> {
        for (path, content) in self.create.iter().chain(&self.update) {
            let path = root.join(path);
            if let Some(dir) = path.parent() {
                fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
            }
            fs::write(&path, content).with_context(|| format!("writing {}", path.display()))?;
        }
        Ok(self.format(root))
    }

    fn paths(&self) -> impl Iterator<Item = &PathBuf> {
        self.create.iter().chain(&self.update).map(|(path, _)| path)
    }

    /// Templates are formatted for typical names; long names can push a
    /// line over the limit, so the touched files go through the formatters.
    fn format(&self, root: &Path) -> Vec<String> {
        let mut warnings = Vec::new();
        let rust: Vec<&PathBuf> = self
            .paths()
            .filter(|p| p.extension().is_some_and(|e| e == "rs"))
            .collect();
        if !rust.is_empty() {
            let status = Command::new("rustfmt")
                .args(["--edition", "2024"])
                .args(&rust)
                .current_dir(root)
                .status();
            if !status.is_ok_and(|s| s.success()) {
                warnings.push("rustfmt failed; run `cargo fmt --all`".to_owned());
            }
        }
        let client: Vec<PathBuf> = self
            .paths()
            .filter_map(|p| p.strip_prefix("clients").ok())
            .filter(|p| {
                p.extension()
                    .is_some_and(|e| e == "ts" || e == "tsx" || e == "json")
            })
            .map(Path::to_path_buf)
            .collect();
        let prettier = root.join("clients/node_modules/.bin/prettier");
        if !client.is_empty() {
            let status = prettier.exists().then(|| {
                Command::new(&prettier)
                    .arg("--write")
                    .arg("--log-level=warn")
                    .args(&client)
                    .current_dir(root.join("clients"))
                    .status()
            });
            if !status.is_some_and(|s| s.is_ok_and(|s| s.success())) {
                warnings.push(
                    "client files not formatted (no `pnpm install` in clients/?); run \
                     `pnpm --dir clients format`"
                        .to_owned(),
                );
            }
        }
        warnings
    }

    /// A one-line-per-file report.
    pub fn report(&self) -> String {
        let mut out = String::new();
        for (path, _) in &self.create {
            let _ = writeln!(out, "  created {}", path.display());
        }
        for (path, _) in &self.update {
            let _ = writeln!(out, "  updated {}", path.display());
        }
        out
    }
}

/// Options of `new-module` that tests pin down.
#[derive(Debug, Clone)]
pub struct ModuleOptions {
    /// Tracking issue URL, recorded in the OpenSpec change.
    pub issue: Option<String>,
    /// Clock for the migration timestamp and the change's `created` date.
    pub now: DateTime<Utc>,
}

/// Plan a new module `name`: its crate, schema migration, OpenSpec change,
/// client module, and its registration in the workspace, the server and the
/// client shell. Refuses if any part of it already exists.
pub fn new_module(root: &Path, name: &str, options: &ModuleOptions) -> Result<Plan> {
    names::validate(Kind::Module, name)?;
    if let Some(issue) = &options.issue
        && !issue.starts_with("https://")
    {
        bail!("--issue must be the tracking issue's https:// URL, got `{issue}`");
    }
    let kebab = names::kebab(name);
    let existing = [
        format!("crates/modules/{name}"),
        format!("clients/app/src/modules/{kebab}"),
        format!("openspec/specs/{name}"),
        format!("openspec/changes/add-{kebab}-module"),
    ];
    for path in &existing {
        if root.join(path).exists() {
            bail!("module `{name}` already exists ({path}); nothing was written");
        }
    }
    let migrations = migration_names(root)?;
    if let Some(found) = migrations
        .iter()
        .find(|m| m.contains(&format!("_{name}__")))
    {
        bail!("module `{name}` already has migrations ({found}); nothing was written");
    }

    let vars = module_vars(
        name,
        &next_timestamp(options.now, &migrations),
        &options.now.format("%Y-%m-%d").to_string(),
        options.issue.as_deref().unwrap_or(NO_ISSUE),
    );
    let mut plan = Plan {
        create: render_module(&vars)?,
        update: Vec::new(),
    };

    for file in REGISTRATION_FILES {
        plan.update.push((
            file.into(),
            register_module(file, &read(root, file)?, name)?,
        ));
    }
    Ok(plan)
}

/// `file`'s text with module `name` registered in it.
fn register_module(file: &str, text: &str, name: &str) -> Result<String> {
    let kebab = names::kebab(name);
    let pascal = names::pascal(name);
    let camel = names::camel(name);
    match file {
        WORKSPACE_MANIFEST => edits::insert_after_last(
            text,
            file,
            "campbooks-",
            "\"crates/modules/",
            &format!("campbooks-{kebab} = {{ path = \"crates/modules/{name}\" }}"),
        ),
        SERVER_MANIFEST => edits::add_dependency(text, file, &format!("campbooks-{kebab}")),
        SERVER_LIB => {
            // Two marked lists: the modules, then the OpenAPI tags.
            let (modules, tags) = split_after_first_marker(text, file)?;
            let title = names::title(name).to_lowercase();
            let modules = edits::insert_before_marker(
                modules,
                file,
                &format!("campbooks_{name}::module(),"),
            )?;
            let tags = edits::insert_before_marker(
                tags,
                file,
                &format!("(name = \"{name}\", description = \"The {title} module.\"),"),
            )?;
            Ok(modules + &tags)
        }
        CLIENT_MODULES => {
            let text = edits::insert_after_last(
                text,
                file,
                "import { build",
                "from \"~/modules/",
                &format!("import {{ build{pascal}Routes }} from \"~/modules/{kebab}\";"),
            )?;
            edits::insert_before_marker(&text, file, &format!("...build{pascal}Routes(root),"))
        }
        CLIENT_I18N_TYPES => {
            let text = edits::insert_after_last(
                text,
                file,
                "import type ",
                "from \"~/modules/",
                &format!("import type {camel} from \"~/modules/{kebab}/i18n/en.json\";"),
            )?;
            // The namespace is the client folder's (kebab-case) name.
            let key = if kebab.contains('-') {
                format!("\"{kebab}\"")
            } else {
                kebab.clone()
            };
            edits::insert_before_marker(&text, file, &format!("{key}: typeof {camel};"))
        }
        _ => bail!("no registration edit for {file}"),
    }
}

/// Plan a new feature slice `feature` in module `module`: its folder with a
/// scoped handler and tests, declared in `features/mod.rs` and mounted on the
/// module's router.
pub fn new_feature(root: &Path, module: &str, feature: &str) -> Result<Plan> {
    names::validate(Kind::Module, module)?;
    names::validate(Kind::Feature, feature)?;
    let src = format!("crates/modules/{module}/src");
    if !root.join(&src).join("lib.rs").exists() {
        bail!(
            "there is no module `{module}` (crates/modules/{module}); create it with `cargo xtask new-module {module}`"
        );
    }
    let dir = format!("{src}/features/{feature}");
    if root.join(&dir).exists() {
        bail!("feature `{feature}` already exists ({dir}); nothing was written");
    }
    let pascal = names::pascal(module);
    let scopes = read(root, &format!("{src}/scopes.rs"))?;
    if !scopes.contains(&format!("pub struct {pascal}Read;")) {
        bail!(
            "{src}/scopes.rs declares no `{pascal}Read` scope, which the generated handler \
             requires; declare `{module}:read` first"
        );
    }

    let vars = feature_vars(module_vars(module, "", "", ""), feature);
    let lib = format!("{src}/lib.rs");
    let mod_rs = format!("{src}/features/mod.rs");
    Ok(Plan {
        create: vec![render_template(&FEATURE_TEMPLATE, &vars)?],
        update: vec![
            (
                lib.clone().into(),
                edits::add_route(&read(root, &lib)?, &lib, feature)?,
            ),
            (
                mod_rs.clone().into(),
                edits::add_feature_mod(&read(root, &mod_rs)?, &mod_rs, feature)?,
            ),
        ],
    })
}

fn module_vars(name: &str, timestamp: &str, date: &str, issue: &str) -> Vars {
    vec![
        ("name", name.to_owned()),
        ("kebab", names::kebab(name)),
        ("pascal", names::pascal(name)),
        ("title", names::title(name)),
        ("title_lower", names::title(name).to_lowercase()),
        ("timestamp", timestamp.to_owned()),
        ("date", date.to_owned()),
        ("issue", issue.to_owned()),
    ]
}

fn feature_vars(mut vars: Vars, feature: &str) -> Vars {
    vars.extend([
        ("feature", feature.to_owned()),
        ("feature_kebab", names::kebab(feature)),
        ("feature_pascal", names::pascal(feature)),
        ("feature_title", names::title(feature)),
        ("feature_title_lower", names::title(feature).to_lowercase()),
    ]);
    vars
}

/// Every file of a new module, its first feature included.
fn render_module(vars: &Vars) -> Result<Vec<(PathBuf, String)>> {
    let feature = feature_vars(vars.clone(), FIRST_FEATURE);
    MODULE_TEMPLATES
        .iter()
        .map(|template| render_template(template, vars))
        .chain([render_template(&FEATURE_TEMPLATE, &feature)])
        .collect()
}

fn render_template(template: &Template, vars: &Vars) -> Result<(PathBuf, String)> {
    let path = substitute(template.path, vars)
        .with_context(|| format!("template path `{}`", template.path))?;
    let body = substitute(template.body, vars).with_context(|| format!("template for {path}"))?;
    Ok((path.into(), body))
}

/// Replace every `{{key}}`; fail on a placeholder no variable fills.
fn substitute(template: &str, vars: &Vars) -> Result<String> {
    let mut out = template.to_owned();
    for (key, value) in vars {
        out = out.replace(&format!("{{{{{key}}}}}"), value);
    }
    if let Some(start) = out.find("{{") {
        let rest = &out[start..];
        let end = rest.find("}}").map_or(rest.len(), |i| i + 2);
        bail!("unknown placeholder `{}`", &rest[..end]);
    }
    Ok(out)
}

/// The text up to and including the first marker line, and the rest.
fn split_after_first_marker<'a>(text: &'a str, file: &str) -> Result<(&'a str, &'a str)> {
    let marker = text
        .find(edits::MARKER)
        .with_context(|| format!("{file} has no `{}` comment", edits::MARKER))?;
    let line_end = text[marker..]
        .find('\n')
        .map_or(text.len(), |i| marker + i + 1);
    Ok(text.split_at(line_end))
}

/// A migration timestamp for `now`, later than every existing migration so
/// the new one sorts (and runs) last.
fn next_timestamp(now: DateTime<Utc>, migrations: &[String]) -> String {
    let latest = migrations
        .iter()
        .filter_map(|name| name.get(..14))
        .filter_map(|ts| NaiveDateTime::parse_from_str(ts, TIMESTAMP_FORMAT).ok())
        .max();
    let now = now.naive_utc();
    let at = match latest {
        Some(latest) if latest >= now => latest + TimeDelta::seconds(1),
        _ => now,
    };
    at.format(TIMESTAMP_FORMAT).to_string()
}

fn migration_names(root: &Path) -> Result<Vec<String>> {
    let dir = root.join("migrations");
    let mut names = Vec::new();
    for entry in fs::read_dir(&dir).with_context(|| format!("reading {}", dir.display()))? {
        names.push(entry?.file_name().to_string_lossy().into_owned());
    }
    Ok(names)
}

fn read(root: &Path, path: &str) -> Result<String> {
    fs::read_to_string(root.join(path)).with_context(|| format!("reading {path}"))
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone as _;

    use super::*;

    const SNAPSHOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/snapshots/new_module.snap");

    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 10, 13, 12, 0, 0).unwrap()
    }

    fn root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .to_path_buf()
    }

    /// Every file `new-module demo_notes` and `new-feature demo_notes
    /// list_items` render, one after the other.
    fn rendered() -> String {
        let vars = module_vars(
            "demo_notes",
            "20261013120000",
            "2026-10-13",
            "https://github.com/notacamp/campbooks-cloud/issues/0",
        );
        let mut files = render_module(&vars).unwrap();
        files.push(render_template(&FEATURE_TEMPLATE, &feature_vars(vars, "list_items")).unwrap());
        files.iter().fold(String::new(), |mut out, (path, body)| {
            let _ = write!(out, "==> {} <==\n{body}\n", path.display());
            out
        })
    }

    /// Snapshot of the rendered templates. After changing a template, review
    /// the diff and refresh with `UPDATE_SNAPSHOTS=1 cargo test -p xtask`.
    #[test]
    fn rendered_files_match_the_snapshot() {
        let fresh = rendered();
        if std::env::var_os("UPDATE_SNAPSHOTS").is_some() {
            fs::write(SNAPSHOT, &fresh).unwrap();
        }
        let committed = fs::read_to_string(SNAPSHOT).unwrap_or_default();
        assert!(
            committed == fresh,
            "rendered templates differ from {SNAPSHOT}; review and refresh with \
             UPDATE_SNAPSHOTS=1 cargo test -p xtask"
        );
    }

    #[test]
    fn every_placeholder_is_filled() {
        assert!(!rendered().contains("{{"));
        let vars = module_vars("x", "", "", "");
        let error = substitute("a {{nope}} b", &vars).unwrap_err().to_string();
        assert!(error.contains("`{{nope}}`"), "{error}");
    }

    #[test]
    fn plans_a_module_against_this_workspace() {
        let options = ModuleOptions {
            issue: None,
            now: now(),
        };
        let plan = new_module(&root(), "demo_notes", &options).unwrap();
        let updated: Vec<String> = plan
            .update
            .iter()
            .map(|(path, _)| path.display().to_string())
            .collect();
        assert_eq!(
            updated,
            [
                WORKSPACE_MANIFEST,
                SERVER_MANIFEST,
                SERVER_LIB,
                CLIENT_MODULES,
                CLIENT_I18N_TYPES
            ]
        );
        let server = &plan.update[2].1;
        assert!(server.contains("campbooks_demo_notes::module(),"));
        assert!(
            server.contains("(name = \"demo_notes\", description = \"The demo notes module.\"),")
        );
        assert!(plan.update[3].1.contains("...buildDemoNotesRoutes(root),"));
        assert!(
            plan.update[4]
                .1
                .contains("\"demo-notes\": typeof demoNotes;")
        );
        let yaml = plan
            .create
            .iter()
            .find(|(path, _)| path.ends_with(".openspec.yaml"))
            .unwrap();
        assert!(yaml.1.contains("issue: TODO(new-module)"));
    }

    #[test]
    fn refuses_an_existing_module() {
        let options = ModuleOptions {
            issue: None,
            now: now(),
        };
        let error = new_module(&root(), "system", &options)
            .unwrap_err()
            .to_string();
        assert!(error.contains("already exists"), "{error}");
        let error = new_module(&root(), "Bad-Name", &options)
            .unwrap_err()
            .to_string();
        assert!(error.contains("snake_case"), "{error}");
        let bad_issue = ModuleOptions {
            issue: Some("#24".into()),
            now: now(),
        };
        assert!(new_module(&root(), "demo_notes", &bad_issue).is_err());
    }

    #[test]
    fn plans_a_feature_in_the_reference_module() {
        let plan = new_feature(&root(), "system", "uptime").unwrap();
        assert_eq!(
            plan.create[0].0,
            PathBuf::from("crates/modules/system/src/features/uptime/mod.rs")
        );
        assert!(plan.create[0].1.contains("Scoped<SystemRead>"));
        assert!(plan.update[0].1.contains("features::uptime::handler"));
        assert!(plan.update[1].1.contains("pub mod uptime;"));

        assert!(new_feature(&root(), "system", "health").is_err());
        assert!(new_feature(&root(), "nowhere", "x").is_err());
    }

    #[test]
    fn timestamps_sort_after_existing_migrations() {
        let migrations = ["20261012100000_kernel__x.sql".to_owned()];
        assert_eq!(next_timestamp(now(), &migrations), "20261013120000");
        let future = [
            "20261013120000_kernel__x.sql".to_owned(),
            "README".to_owned(),
        ];
        assert_eq!(next_timestamp(now(), &future), "20261013120001");
    }
}
