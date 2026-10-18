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
//!
//! Every export is bounded by one member's visibility (campbooks-cloud#219):
//! `--member` names them (their id, or their verified address), and only
//! the inboxes they can read are exported, as the API decides it; `--inbox`
//! narrows it to one of those. There is no "whole workspace". The folder
//! carries a marker ([`MARKER`]), and `campbooks-server labelling purge
//! --out <folder>` removes a folder only when it has one.

use std::{
    fs,
    io::Write as _,
    path::{Path, PathBuf},
    process::ExitCode,
};

use campbooks_ai::decide::signals::actions::ActionId;
use campbooks_conversations::labelling::{
    LabellingItem, LabellingSet, MAX_ITEMS, export_set, manifest_for,
};
use campbooks_kernel::{
    AppState, Config, Error,
    db::BeginFor as _,
    tenancy::{Access, WorkspaceId},
};
use clap::Subcommand;

/// The file every export folder carries, so `purge` removes only a folder
/// this tool wrote. Its first line is [`MARKER_HEADER`].
pub const MARKER: &str = ".campbooks-labelling";

/// The first line of [`MARKER`].
const MARKER_HEADER: &str = "campbooks-server labelling export";

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
        /// The member whose visibility bounds the export: their member id,
        /// or their verified address. Only the inboxes they can read are
        /// exported (never a teammate's private inbox).
        #[arg(long)]
        member: String,
        /// Only this inbox (its id), which the member must be able to read.
        #[arg(long)]
        inbox: Option<String>,
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
    /// Remove a folder `export` wrote (it carries a `.campbooks-labelling`
    /// marker); refuses any other folder.
    Purge {
        /// The folder to remove.
        #[arg(long)]
        out: PathBuf,
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

    /// Its name on the command line.
    const fn name(self) -> &'static str {
        match self {
            Self::Money => "money",
            Self::Actions => "actions",
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
    match command {
        LabellingCommand::Export {
            workspace,
            member,
            inbox,
            days,
            out,
            set,
        } => export(&Export {
            workspace,
            member,
            inbox,
            days,
            out,
            set,
        }),
        LabellingCommand::Purge { out } => match purge(&out) {
            Ok(removed) => {
                let _ = writeln!(std::io::stdout(), "removed {}", removed.display());
                ExitCode::SUCCESS
            }
            Err(e) => super::fail(&e),
        },
    }
}

/// What `export` was asked.
struct Export {
    workspace: String,
    member: String,
    inbox: Option<String>,
    days: u32,
    out: PathBuf,
    set: Set,
}

fn export(args: &Export) -> ExitCode {
    let Ok(workspace) = args.workspace.parse::<WorkspaceId>() else {
        return super::fail("--workspace must be a workspace id (a UUID)");
    };
    let inbox = match args.inbox.as_deref().map(str::parse) {
        None => None,
        Some(Ok(inbox)) => Some(inbox),
        Some(Err(_)) => return super::fail("--inbox must be an inbox id (a UUID)"),
    };
    let out = &args.out;
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
    let set = args.set;
    let exported = runtime.block_on(async {
        let db = super::connect(&config).await?;
        let blobs = campbooks_kernel::blobs::BlobStore::from_config(&config.blobs, db.clone())?;
        let state = AppState::new(db.clone(), config).with_blobs(blobs);
        let items = bounded_export(&state, workspace, &args.member, inbox, args.days, set).await;
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
    if let Err(e) = write(out, &items, set, workspace) {
        return super::fail(&format!(
            "could not write {}: {e} (remove what was written with \
             campbooks-server labelling purge --out {})",
            out.display(),
            out.display()
        ));
    }
    let at = fs::canonicalize(out).unwrap_or_else(|_| out.clone());
    let next = match set {
        Set::Money => format!(
            "Fill the `money` column of labels.csv with owed, paid or neither, then run\n  \
             cargo xtask eval decide --set-dir {} --question money --strategy jev",
            at.display()
        ),
        Set::Actions => format!(
            "For each email, write yes under every action you would take (leave the others\n\
             blank; a row left all blank is skipped), then run\n  \
             cargo xtask eval actions --set-dir {} --floor 0.6 --floor 0.7 --floor 0.8",
            at.display()
        ),
    };
    let _ = writeln!(
        std::io::stdout(),
        "wrote {} message(s) to {}\n{next}\n\
         This folder holds real mail: keep it on your own machine and never commit it.\n\
         Delete it from this server once copied: campbooks-server labelling purge --out {}",
        items.len(),
        at.display(),
        at.display(),
    );
    ExitCode::SUCCESS
}

/// The export of `set`, bounded by the member `who` names (and `inbox`).
async fn bounded_export(
    state: &AppState,
    workspace: WorkspaceId,
    who: &str,
    inbox: Option<campbooks_conversations::model::InboxId>,
    days: u32,
    set: Set,
) -> Result<Vec<LabellingItem>, String> {
    let mut tx = state
        .db
        .begin_for(workspace)
        .await
        .map_err(|e| e.to_string())?;
    let named = campbooks_workspaces::operator::members_named(&mut tx, workspace, who)
        .await
        .map_err(|e| e.to_string())?;
    tx.commit().await.map_err(|e| e.to_string())?;
    let (user, role) = match named.as_slice() {
        [one] => *one,
        [] => {
            return Err(format!(
                "nobody in workspace {workspace} is {who}: name a member by their member id \
                 or their verified address"
            ));
        }
        several => {
            let ids: Vec<String> = several.iter().map(|(user, _)| user.to_string()).collect();
            return Err(format!(
                "{} members have the address {who}: choose one with --member <id>: {}",
                several.len(),
                ids.join(", ")
            ));
        }
    };
    let access = Access {
        workspace,
        user: Some(user),
        role: Some(role),
    };
    export_set(state, &access, inbox, days, set.labelling())
        .await
        .map_err(|e| match (e, inbox) {
            (Error::NotFound, Some(inbox)) => format!(
                "{who} cannot read inbox {inbox}: name one of their inboxes, or leave --inbox out"
            ),
            (Error::NotFound, None) => {
                format!("{who} reads no inbox in workspace {workspace}: nothing to export")
            }
            (e, _) => e.to_string(),
        })
}

/// What an export folder may hold: what `export` writes, and the reports
/// `cargo xtask eval` writes beside it. `purge` removes nothing else.
const FOLDER_FILES: [&str; 3] = [MARKER, "manifest.json", "labels.csv"];
const FOLDER_DIRS: [&str; 3] = ["items", "report", "reports"];

/// Remove `out`, a folder `export` wrote: a real folder (not a link) whose
/// [`MARKER`] starts with [`MARKER_HEADER`], holding only what an export and
/// its eval write (no links anywhere). Anything else is refused, and nothing
/// is removed. Every check is made on the folder's resolved path, the one
/// removed.
fn purge(out: &Path) -> Result<PathBuf, String> {
    let named = fs::symlink_metadata(out).map_err(|e| format!("{}: {e}", out.display()))?;
    if !named.is_dir() {
        return Err(format!(
            "{} is not a folder: nothing removed",
            out.display()
        ));
    }
    let at = fs::canonicalize(out).map_err(|e| format!("{}: {e}", out.display()))?;
    let marker = at.join(MARKER);
    let written = fs::symlink_metadata(&marker).is_ok_and(|m| m.is_file())
        && fs::read_to_string(&marker).is_ok_and(|text| text.lines().next() == Some(MARKER_HEADER));
    if !written {
        return Err(format!(
            "{} was not written by `campbooks-server labelling export` (no {MARKER} marker): \
             nothing removed",
            at.display()
        ));
    }
    let strangers = strangers(&at).map_err(|e| format!("{}: {e}", at.display()))?;
    if !strangers.is_empty() {
        return Err(format!(
            "{} holds more than an export ({}): nothing removed",
            at.display(),
            strangers.join(", ")
        ));
    }
    fs::remove_dir_all(&at).map_err(|e| format!("could not remove {}: {e}", at.display()))?;
    Ok(at)
}

/// What in `folder` an export and its eval did not write: unknown names, a
/// link, or a folder holding anything but files.
fn strangers(folder: &Path) -> std::io::Result<Vec<String>> {
    let mut out = Vec::new();
    for entry in fs::read_dir(folder)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let kind = entry.file_type()?;
        if kind.is_file() && FOLDER_FILES.contains(&name.as_str()) {
            continue;
        }
        if kind.is_dir() && FOLDER_DIRS.contains(&name.as_str()) {
            for inner in fs::read_dir(entry.path())? {
                let inner = inner?;
                if !inner.file_type()?.is_file() {
                    out.push(format!("{name}/{}", inner.file_name().to_string_lossy()));
                }
            }
            continue;
        }
        out.push(name);
    }
    out.sort();
    Ok(out)
}

fn write(
    out: &Path,
    items: &[LabellingItem],
    set: Set,
    workspace: WorkspaceId,
) -> std::io::Result<()> {
    // Real mail: readable by the operator only, and never into a folder
    // that exists (no labels overwritten, no race with the check above).
    private_dir(out)?;
    // First, so `purge` can always remove what was started.
    private_file(
        &out.join(MARKER),
        &format!(
            "{MARKER_HEADER}\nworkspace {workspace}\nset {}\n",
            set.name()
        ),
    )?;
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
    use clap::Parser;

    use super::{LabellingCommand, MARKER, Set, csv_cell, purge};

    /// The `labelling` subcommands, as the server parses them.
    #[derive(Parser)]
    struct Cli {
        #[command(subcommand)]
        command: LabellingCommand,
    }

    /// A new, empty folder of this test's own.
    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "campbooks-labelling-{name}-{}-{}",
            std::process::id(),
            uuid::Uuid::now_v7()
        ));
        std::fs::create_dir(&dir).expect("scratch folder");
        dir
    }

    #[test]
    fn an_export_names_the_member_whose_visibility_bounds_it() {
        let workspace = "0192f1a0-0000-7000-8000-000000000001";
        let without = Cli::try_parse_from([
            "labelling",
            "export",
            "--workspace",
            workspace,
            "--out",
            "/tmp/owner-money",
        ]);
        let err = without.err().expect("refused without --member");
        assert!(err.to_string().contains("--member"), "{err}");
        let with = Cli::try_parse_from([
            "labelling",
            "export",
            "--workspace",
            workspace,
            "--member",
            "sam@example.com",
            "--inbox",
            "0192f1a0-0000-7000-8000-000000000002",
            "--set",
            "actions",
            "--out",
            "/tmp/owner-actions",
        ]);
        assert!(with.is_ok());
    }

    #[test]
    fn purge_removes_only_a_folder_export_wrote() {
        // No marker: refused, nothing removed.
        let other = scratch("other");
        std::fs::write(other.join("notes.txt"), "keep me").expect("a file");
        let refused = purge(&other).expect_err("no marker");
        assert!(refused.contains(MARKER), "{refused}");
        assert!(other.join("notes.txt").exists());
        // A marker that is not ours: refused too.
        std::fs::write(other.join(MARKER), "something else\n").expect("a marker");
        assert!(purge(&other).is_err());
        assert!(other.join("notes.txt").exists());
        std::fs::remove_dir_all(&other).expect("cleaned up");

        // A file, not a folder: refused.
        let file =
            std::env::temp_dir().join(format!("campbooks-labelling-file-{}", uuid::Uuid::now_v7()));
        std::fs::write(&file, "x").expect("a file");
        assert!(purge(&file).is_err());
        std::fs::remove_file(&file).expect("cleaned up");

        // A marker, but more than an export holds: refused.
        let mixed = scratch("mixed");
        std::fs::write(
            mixed.join(MARKER),
            "campbooks-server labelling export\nworkspace w\nset money\n",
        )
        .expect("a marker");
        std::fs::write(mixed.join("thesis.docx"), "mine").expect("a file");
        let refused = purge(&mixed).expect_err("a stranger");
        assert!(refused.contains("thesis.docx"), "{refused}");
        assert!(mixed.join("thesis.docx").exists());
        std::fs::remove_dir_all(&mixed).expect("cleaned up");

        // An export's folder: removed, with what it holds.
        let ours = scratch("ours");
        std::fs::write(
            ours.join(MARKER),
            "campbooks-server labelling export\nworkspace w\nset money\n",
        )
        .expect("a marker");
        std::fs::create_dir(ours.join("items")).expect("items");
        std::fs::write(ours.join("items").join("owner.jsonl"), "{}\n").expect("items");
        std::fs::write(ours.join("labels.csv"), "id\n").expect("labels");
        std::fs::create_dir(ours.join("report")).expect("report");
        std::fs::write(ours.join("report").join("actions.md"), "# report\n").expect("report");
        let removed = purge(&ours).expect("purged");
        assert!(!removed.exists());
        assert!(!ours.exists());
    }

    #[cfg(unix)]
    #[test]
    fn purge_never_follows_a_link() {
        let ours = scratch("linked");
        std::fs::write(
            ours.join(MARKER),
            "campbooks-server labelling export\nworkspace w\nset money\n",
        )
        .expect("a marker");
        let link =
            std::env::temp_dir().join(format!("campbooks-labelling-link-{}", uuid::Uuid::now_v7()));
        std::os::unix::fs::symlink(&ours, &link).expect("a link");
        assert!(purge(&link).is_err());
        assert!(ours.join(MARKER).exists());
        std::fs::remove_file(&link).expect("cleaned up");
        std::fs::remove_dir_all(&ours).expect("cleaned up");
    }

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
