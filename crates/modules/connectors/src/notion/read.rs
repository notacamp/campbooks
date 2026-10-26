//! Reading a Notion page's blocks into a [`Snapshot`].
//!
//! The block tree is read level by level through the API (a hundred blocks
//! a call, each call taking the connection's budget), at most `max_blocks`
//! blocks and `max_depth` levels deep. Blocks Pages lacks are kept as their
//! Notion type and nothing of their content, and their children are not
//! read at all. A page over the limits is read in part and says so
//! (`complete: false`), so blocks not seen are never taken for blocks that
//! went away. Files Notion hosts are downloaded (their addresses expire
//! within the hour) into the workspace's blobs, at most 20 MiB each and of
//! an allowed type; one that cannot be kept becomes an `external` block.

use std::collections::BTreeMap;

use campbooks_kernel::blobs::{BlobRef, BlobStore};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{
    Notion, PageInfo,
    blocks::{self, Block, FileRef, FileWant, Kind, Snapshot},
    pace::{Pace, Stop},
};

/// Most downloaded files a stream remembers the blobs of.
pub const MAX_KNOWN_FILES: usize = 200;

/// The files a stream has downloaded, so an unchanged one is not downloaded
/// again at every read.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Files {
    /// By the block's Notion id.
    #[serde(default)]
    pub known: BTreeMap<String, KnownFile>,
}

/// A downloaded file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnownFile {
    /// When Notion last edited the block it was downloaded for.
    pub edited: DateTime<Utc>,
    /// Where it is kept.
    pub blob: BlobRef,
    /// Its name.
    pub name: String,
}

/// How much of a page is read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Most blocks.
    pub max_blocks: usize,
    /// Deepest levels.
    pub max_depth: usize,
}

/// What a read gave.
#[derive(Debug)]
pub struct Read {
    /// The page's blocks.
    pub snapshot: Snapshot,
    /// Every blob the snapshot names, to be held by whoever keeps it.
    pub held: Vec<BlobRef>,
    /// The downloaded files, as remembered now.
    pub files: Files,
}

/// A read in progress.
struct Reader<'a> {
    notion: &'a Notion,
    token: &'a str,
    pace: &'a mut Pace,
    blobs: &'a BlobStore,
    workspace: &'a str,
    limits: Limits,
    files: Files,
    seen: usize,
    complete: bool,
    held: Vec<BlobRef>,
}

/// Read the blocks of `page` as they are now.
#[allow(
    clippy::too_many_arguments,
    reason = "one call site; the values are distinct"
)]
pub async fn read_page(
    notion: &Notion,
    token: &str,
    pace: &mut Pace,
    blobs: &BlobStore,
    workspace: &str,
    page: &PageInfo,
    files: Files,
    limits: Limits,
    now: DateTime<Utc>,
) -> Result<Read, Stop> {
    let mut reader = Reader {
        notion,
        token,
        pace,
        blobs,
        workspace,
        limits,
        files,
        seen: 0,
        complete: true,
        held: Vec::new(),
    };
    let blocks = reader.children(page.id, 1).await?;
    // Only the files the page still names are remembered.
    let mut named = Vec::new();
    collect_files(&blocks, &mut named);
    reader.files.known.retain(|id, _| named.contains(id));
    Ok(Read {
        snapshot: Snapshot {
            version: 1,
            notion_page: page.id,
            title: page.title.clone(),
            edited: page.edited,
            read_at: now,
            complete: reader.complete,
            blocks,
        },
        held: reader.held,
        files: reader.files,
    })
}

fn collect_files(blocks: &[Block], out: &mut Vec<String>) {
    for block in blocks {
        if block.file.is_some() {
            out.push(block.id.to_string());
        }
        collect_files(&block.children, out);
    }
}

impl Reader<'_> {
    /// The blocks under `parent`, `depth` levels down (a page is level 1).
    async fn children(&mut self, parent: Uuid, depth: usize) -> Result<Vec<Block>, Stop> {
        let mut out = Vec::new();
        let mut cursor: Option<String> = None;
        loop {
            if self.seen >= self.limits.max_blocks {
                self.complete = false;
                break;
            }
            self.pace.take().await?;
            let listed = self
                .notion
                .children(self.token, parent, cursor.as_deref())
                .await?;
            for raw in &listed.blocks {
                if self.seen >= self.limits.max_blocks {
                    self.complete = false;
                    break;
                }
                let Some(parsed) = blocks::parse(raw) else {
                    continue;
                };
                self.seen += 1;
                let mut block = parsed.block;
                if let Some(want) = &parsed.file {
                    self.keep_file(&mut block, want).await?;
                }
                if parsed.recurse {
                    if depth >= self.limits.max_depth {
                        self.complete = false;
                    } else {
                        block.children = Box::pin(self.children(block.id, depth + 1)).await?;
                    }
                }
                out.push(block);
            }
            match listed.next {
                Some(next) => cursor = Some(next),
                None => break,
            }
        }
        Ok(out)
    }

    /// Download the file `block` wants into the workspace's blobs, or turn
    /// the block into one kept in Notion when it cannot be had.
    async fn keep_file(&mut self, block: &mut Block, want: &FileWant) -> Result<(), Stop> {
        let id = block.id.to_string();
        if let Some(known) = self.files.known.get(&id).cloned()
            && Some(known.edited) == block.edited
            && self
                .blobs
                .head(self.workspace, &known.blob.hash)
                .await
                .ok()
                .flatten()
                .is_some()
        {
            block.file = Some(file_ref(&known.blob, &known.name));
            self.held.push(known.blob);
            return Ok(());
        }
        let Some(file) = self.notion.download(&want.url).await? else {
            demote(block);
            return Ok(());
        };
        let blob = self
            .blobs
            .put(self.workspace, file.bytes, &file.content_type)
            .await
            .map_err(|e| Stop::Database(e.into()))?;
        block.file = Some(file_ref(&blob, &want.name));
        if let Some(edited) = block.edited
            && self.files.known.len() < MAX_KNOWN_FILES
        {
            self.files.known.insert(
                id,
                KnownFile {
                    edited,
                    blob: blob.clone(),
                    name: want.name.clone(),
                },
            );
        }
        self.held.push(blob);
        Ok(())
    }
}

fn file_ref(blob: &BlobRef, name: &str) -> FileRef {
    FileRef {
        blob: blob.hash.to_string(),
        size: blob.size,
        content_type: blob.content_type.clone(),
        name: name.to_owned(),
    }
}

/// A file that cannot be kept stays in Notion.
fn demote(block: &mut Block) {
    let label = if block.kind == Kind::Image {
        "image"
    } else {
        "file"
    };
    block.kind = Kind::External;
    block.label = Some(label.to_owned());
    block.file = None;
}

#[cfg(test)]
mod tests;
