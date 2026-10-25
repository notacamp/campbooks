//! `campbooks-tracking-plan` — check, format, patch and generate from the
//! tracking plan (ADR 0020). See the library's documentation.

#![forbid(unsafe_code)]

use std::io::Write as _;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "campbooks-tracking-plan",
    about = "The tracking plan's tooling: check, generate, fmt, apply, init."
)]
struct Cli {
    /// The repository root (default: the one this tool was built in).
    #[arg(long)]
    root: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Check the plan, that the generated files are current, that live
    /// events are tracked outside tests and that Coverage names real
    /// operations.
    Check,
    /// Write tracking/tracking-plan.json and every tracking.gen.rs.
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

fn main() -> ExitCode {
    let cli = Cli::parse();
    let root = cli
        .root
        .unwrap_or_else(campbooks_tracking_plan::default_root);
    let result = match cli.command {
        Command::Check => campbooks_tracking_plan::check(&root).map(|report| {
            let mut out = String::new();
            if report.problems.is_empty() {
                out.push_str("tracking plan OK");
                Ok(out)
            } else {
                for problem in &report.problems {
                    out.push_str("  - ");
                    out.push_str(problem);
                    out.push('\n');
                }
                Err(format!(
                    "{out}{} tracking plan problem(s) (see docs/tracking.md)",
                    report.problems.len()
                ))
            }
        }),
        Command::Generate => campbooks_tracking_plan::generate(&root).map(Ok),
        Command::Fmt => campbooks_tracking_plan::fmt(&root).map(Ok),
        Command::Apply { patch } => campbooks_tracking_plan::apply(&root, &patch).map(Ok),
        Command::Init => campbooks_tracking_plan::init(&root).map(Ok),
    };
    match result {
        Ok(Ok(message)) => {
            let _ = writeln!(std::io::stdout(), "{message}");
            ExitCode::SUCCESS
        }
        Ok(Err(message)) => {
            let _ = writeln!(std::io::stderr(), "{message}");
            ExitCode::FAILURE
        }
        Err(e) => {
            let _ = writeln!(std::io::stderr(), "error: {e:#}");
            ExitCode::FAILURE
        }
    }
}
