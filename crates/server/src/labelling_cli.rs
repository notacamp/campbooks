//! `campbooks-server labelling export` (campbooks-cloud#167): a workspace's
//! recent money mail, written as an eval set the owner labels on their own
//! machine. It runs with shell access to the server, on the runtime login
//! in the workspace (row-level security applies), and writes a folder:
//!
//! - `manifest.json`: the set (`owner-money`, private, the `money` question);
//! - `items/owner.jsonl`: one message per line, as the decide model reads it;
//! - `labels.csv`: one row per message (id, date, sender, subject,
//!   attachments) with an empty `money` column to fill with `owed`, `paid`
//!   or `neither`.
//!
//! The folder holds real mail: it is the owner's, it is never committed,
//! and `cargo xtask eval decide --set-dir <folder>` reads it where it is.

use std::{fs, io::Write as _, path::Path, process::ExitCode};

use campbooks_conversations::labelling::{LabellingItem, export_for_labelling, manifest};
use campbooks_kernel::{AppState, Config, tenancy::WorkspaceId};
use clap::Subcommand;

/// What `campbooks-server labelling` does.
#[derive(Debug, Subcommand)]
pub enum LabellingCommand {
    /// Write a workspace's recent money mail (conversations in Receipts, or
    /// that had a Pay need) as an unlabelled eval set, for its owner to
    /// label locally. Real mail: never commit the folder.
    Export {
        /// The workspace's id.
        #[arg(long)]
        workspace: String,
        /// How many days back.
        #[arg(long, default_value_t = 90)]
        days: u32,
        /// The folder to write (created; must not exist yet).
        #[arg(long)]
        out: std::path::PathBuf,
    },
}

/// Run `command`.
pub fn run(command: LabellingCommand) -> ExitCode {
    let LabellingCommand::Export {
        workspace,
        days,
        out,
    } = command;
    let Ok(workspace) = workspace.parse::<WorkspaceId>() else {
        return super::fail("--workspace must be a workspace id (a UUID)");
    };
    if out.exists() {
        return super::fail(&format!(
            "{} exists already: name a new folder, so no labels are overwritten",
            out.display()
        ));
    }
    let config = match Config::from_env() {
        Ok(config) => config,
        Err(e) => return super::fail(&e.to_string()),
    };
    let runtime = match super::runtime() {
        Ok(runtime) => runtime,
        Err(e) => return super::fail(&e),
    };
    let exported = runtime.block_on(async {
        let db = super::connect(&config).await?;
        let blobs = campbooks_kernel::blobs::BlobStore::from_config(&config.blobs, db.clone())?;
        let state = AppState::new(db.clone(), config).with_blobs(blobs);
        let items = export_for_labelling(&state, workspace, days)
            .await
            .map_err(|e| e.to_string());
        db.close().await;
        items
    });
    let items = match exported {
        Ok(items) => items,
        Err(e) => return super::fail(&e),
    };
    if let Err(e) = write(&out, &items) {
        return super::fail(&format!("could not write {}: {e}", out.display()));
    }
    let _ = writeln!(
        std::io::stdout(),
        "wrote {} message(s) to {}\n\
         Fill the `money` column of labels.csv with owed, paid or neither, then run\n  \
         cargo xtask eval decide --set-dir {} --question money --strategy jev\n\
         This folder holds real mail: keep it on your own machine and never commit it.",
        items.len(),
        out.display(),
        out.display()
    );
    ExitCode::SUCCESS
}

fn write(out: &Path, items: &[LabellingItem]) -> std::io::Result<()> {
    fs::create_dir_all(out.join("items"))?;
    let manifest = serde_json::to_string_pretty(&manifest()).map_err(std::io::Error::other)?;
    fs::write(out.join("manifest.json"), manifest + "\n")?;
    let mut lines = String::new();
    for item in items {
        lines.push_str(&item.item.to_string());
        lines.push('\n');
    }
    fs::write(out.join("items").join("owner.jsonl"), lines)?;
    let mut csv = String::from("id,date,from,subject,attachments,money\n");
    for item in items {
        let row = [
            item.id.clone(),
            item.sent_at.format("%Y-%m-%d %H:%M").to_string(),
            item.from.clone(),
            item.subject.clone(),
            item.attachments.join("; "),
            String::new(),
        ];
        let cells: Vec<String> = row.iter().map(|cell| csv_cell(cell)).collect();
        csv.push_str(&cells.join(","));
        csv.push('\n');
    }
    fs::write(out.join("labels.csv"), csv)
}

/// A CSV cell: quoted, with quotes doubled and line breaks as spaces. A
/// cell a spreadsheet would read as a formula (a subject starting with `=`,
/// `+`, `-` or `@`, written by the sender) starts with `'`, so opening the
/// sheet runs nothing.
fn csv_cell(text: &str) -> String {
    let flat: String = text
        .chars()
        .map(|c| if c == '\n' || c == '\r' { ' ' } else { c })
        .collect();
    let safe = if flat.starts_with(['=', '+', '-', '@', '\t']) {
        format!("'{flat}")
    } else {
        flat
    };
    format!("\"{}\"", safe.replace('"', "\"\""))
}

#[cfg(test)]
mod tests {
    use super::csv_cell;

    #[test]
    fn cells_are_quoted() {
        assert_eq!(csv_cell("a"), "\"a\"");
        assert_eq!(csv_cell("Re: \"x\", y\nz"), "\"Re: \"\"x\"\", y z\"");
        assert_eq!(csv_cell("=HYPERLINK(1)"), "\"'=HYPERLINK(1)\"");
    }
}
