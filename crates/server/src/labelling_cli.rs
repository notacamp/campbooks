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
//! `--set actions` (campbooks-cloud#166) exports the mail that had a need
//! instead, with one empty column per action of the catalogue for the owner
//! to mark (`yes`, or left blank for no), scored by `cargo xtask eval
//! actions`. The same guards apply: operator only, a new private folder,
//! real mail never committed.
//!
//! The folder holds real mail: it is the owner's, it is never committed,
//! and `cargo xtask eval decide --set-dir <folder>` reads it where it is.

use std::{fs, io::Write as _, path::Path, process::ExitCode};

use campbooks_ai::decide::signals::actions::ActionId;
use campbooks_conversations::labelling::{
    LabellingItem, LabellingSet, MAX_ITEMS, export_set, manifest_for,
};
use campbooks_kernel::{AppState, Config, tenancy::WorkspaceId};
use clap::Subcommand;

/// What `campbooks-server labelling` does.
#[derive(Debug, Subcommand)]
pub enum LabellingCommand {
    /// Write a workspace's recent money mail (conversations in Receipts, or
    /// with a Pay need) as an unlabelled eval set, for its owner to label
    /// locally. Real mail: never commit the folder.
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
        /// Which set: `money` (the money question) or `actions` (the action
        /// catalogue: mail that had a need, one column per action).
        #[arg(long, value_enum, default_value_t = Set::Money)]
        set: Set,
    },
}

/// Which eval set an export is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Set {
    /// The money question (campbooks-cloud#167).
    Money,
    /// The action catalogue (campbooks-cloud#166).
    Actions,
}

impl Set {
    const fn labelling(self) -> LabellingSet {
        match self {
            Self::Money => LabellingSet::Money,
            Self::Actions => LabellingSet::Actions,
        }
    }

    /// The columns the owner fills in.
    fn columns(self) -> Vec<&'static str> {
        match self {
            Self::Money => vec!["money"],
            Self::Actions => ActionId::ALL.iter().map(|a| a.as_str()).collect(),
        }
    }
}

/// Run `command`.
pub fn run(command: LabellingCommand) -> ExitCode {
    let LabellingCommand::Export {
        workspace,
        days,
        out,
        set,
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
        let items = export_set(&state, workspace, days, set.labelling())
            .await
            .map_err(|e| e.to_string());
        db.close().await;
        items
    });
    let items = match exported {
        Ok(items) => items,
        Err(e) => return super::fail(&e),
    };
    if items.len() >= usize::try_from(MAX_ITEMS).unwrap_or(usize::MAX) {
        let _ = writeln!(
            std::io::stdout(),
            "only the newest {MAX_ITEMS} messages: use fewer --days for an older window"
        );
    }
    if let Err(e) = write(&out, &items, set) {
        return super::fail(&format!("could not write {}: {e}", out.display()));
    }
    let next = match set {
        Set::Money => format!(
            "Fill the `money` column of labels.csv with owed, paid or neither, then run\n  \
             cargo xtask eval decide --set-dir {} --question money --strategy jev",
            out.display()
        ),
        Set::Actions => format!(
            "For each email, write yes under every action you would take (leave the others\n\
             blank; a row left all blank is skipped), then run\n  \
             cargo xtask eval actions --set-dir {} --floor 0.6 --floor 0.7 --floor 0.8",
            out.display()
        ),
    };
    let _ = writeln!(
        std::io::stdout(),
        "wrote {} message(s) to {}\n{next}\n\
         This folder holds real mail: keep it on your own machine and never commit it.",
        items.len(),
        out.display(),
    );
    ExitCode::SUCCESS
}

fn write(out: &Path, items: &[LabellingItem], set: Set) -> std::io::Result<()> {
    // Real mail: readable by the operator only, and never into a folder
    // that exists (no labels overwritten, no race with the check above).
    private_dir(out)?;
    private_dir(&out.join("items"))?;
    let manifest = serde_json::to_string_pretty(&manifest_for(set.labelling()))
        .map_err(std::io::Error::other)?;
    private_file(&out.join("manifest.json"), &(manifest + "\n"))?;
    let mut lines = String::new();
    for item in items {
        lines.push_str(&item.item.to_string());
        lines.push('\n');
    }
    private_file(&out.join("items").join("owner.jsonl"), &lines)?;
    let columns = set.columns();
    let mut csv = format!("id,date,from,subject,attachments,{}\n", columns.join(","));
    for item in items {
        let mut row = vec![
            item.id.clone(),
            item.sent_at.format("%Y-%m-%d %H:%M").to_string(),
            item.from.clone(),
            item.subject.clone(),
            item.attachments.join("; "),
        ];
        row.extend(columns.iter().map(|_| String::new()));
        let cells: Vec<String> = row.iter().map(|cell| csv_cell(cell)).collect();
        csv.push_str(&cells.join(","));
        csv.push('\n');
    }
    private_file(&out.join("labels.csv"), &csv)
}

/// Create `dir`, which must not exist, readable by its owner only.
fn private_dir(dir: &Path) -> std::io::Result<()> {
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
    builder.create(dir)
}

/// Write a new file readable by its owner only.
fn private_file(path: &Path, contents: &str) -> std::io::Result<()> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    options.open(path)?.write_all(contents.as_bytes())
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
    use super::{Set, csv_cell};

    #[test]
    fn each_set_has_its_own_columns() {
        assert_eq!(Set::Money.columns(), ["money"]);
        assert_eq!(
            Set::Actions.columns(),
            [
                "reply",
                "remind",
                "open_attachment",
                "forward",
                "report",
                "safe",
                "already_paid",
                "still_to_pay"
            ]
        );
    }

    #[test]
    fn cells_are_quoted() {
        assert_eq!(csv_cell("a"), "\"a\"");
        assert_eq!(csv_cell("Re: \"x\", y\nz"), "\"Re: \"\"x\"\", y z\"");
        assert_eq!(csv_cell("=HYPERLINK(1)"), "\"'=HYPERLINK(1)\"");
    }
}
