//! `cargo xtask tracking [--check | generate | fmt | apply <patch> | init]`:
//! the tracking plan's tooling (ADR 0020), a thin wrapper over the
//! `campbooks-tracking-plan` crate, which CI also runs on its own
//! (`cargo run -p campbooks-tracking-plan -- check`) without building the
//! server.

use std::path::{Path, PathBuf};

use anyhow::{Result, bail};
use clap::Subcommand;

/// What `cargo xtask tracking` does besides generating.
#[derive(Debug, Subcommand)]
pub enum TrackingCommand {
    /// Write tracking/tracking-plan.json and every tracking.gen.rs (the
    /// default).
    Generate,
    /// Rewrite the workbook in canonical form.
    Fmt,
    /// Add or update rows from a YAML patch, then format and generate.
    Apply {
        /// The patch, e.g. tracking/patches/0002-things.yaml.
        patch: PathBuf,
    },
    /// Write a new, empty workbook.
    Init,
}

/// Run it: `check` checks; otherwise `command` (generate by default).
pub fn run(root: &Path, check: bool, command: Option<TrackingCommand>) -> Result<String> {
    if check {
        let report = campbooks_tracking_plan::check(root)?;
        let mut out = String::new();
        for warning in &report.warnings {
            out.push_str("warning: ");
            out.push_str(warning);
            out.push('\n');
        }
        if report.problems.is_empty() {
            out.push_str("tracking plan OK");
            return Ok(out);
        }
        let list: Vec<String> = report.problems.iter().map(|p| format!("  - {p}")).collect();
        bail!(
            "{out}{} tracking plan problem(s) (see docs/tracking.md):\n{}",
            report.problems.len(),
            list.join("\n")
        );
    }
    match command.unwrap_or(TrackingCommand::Generate) {
        TrackingCommand::Generate => campbooks_tracking_plan::generate(root),
        TrackingCommand::Fmt => campbooks_tracking_plan::fmt(root),
        TrackingCommand::Apply { patch } => campbooks_tracking_plan::apply(root, &patch),
        TrackingCommand::Init => campbooks_tracking_plan::init(root),
    }
}
