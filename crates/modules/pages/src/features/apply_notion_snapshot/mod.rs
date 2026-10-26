//! `pages.apply_notion_snapshot` ← `connectors.notion_page_read`: a Notion
//! page's blocks were read, and are applied to the page linked to it,
//! block by block (campbooks-cloud#376).
//!
//! In one transaction that holds the page's row like a sync does:
//!
//! 1. The page and its link must be as the event says (syncing, the same
//!    connection and Notion page), and the snapshot newer than the last one
//!    applied; anything else is ignored.
//! 2. Sub-pages the snapshot names that are not pages here yet are made and
//!    linked, each its own Notion page to read.
//! 3. The snapshot is merged into the body (`domain::notion::merge`): what
//!    changed only at Notion is written in, what changed only here stays,
//!    what changed on both sides is kept apart as a conflict. The result is
//!    read against the block schema like any `pages.sync`; a snapshot that
//!    would make a body the schema refuses is refused whole.
//! 4. The files the new blocks name are held by the page, the bases the last
//!    sync agreed are moved, the link's counters and the title are kept, and
//!    the change is announced and tracked.
//!
//! No text reaches an event, a log or a job error: they carry ids and
//! counts.

use campbooks_kernel::{
    AppState, Event, EventHandler, JobContext, JobError,
    blobs::{BlobHash, BlobOwner, BlobRef},
    tenancy::{Access, WorkspaceId},
};
use futures_util::StreamExt as _;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use crate::{
    domain::{
        MAX_DEPTH, PageId, PageRow, Place, SELECT_LOCKED, announce, body, clean_title,
        notion::{
            self, Link, MAX_SNAPSHOT_BYTES,
            merge::merge_into,
            model::{FileRef, Kind, Snapshot},
        },
        position, roles, store, tracked,
    },
    events::{NotionLinkRequested, NotionPageRead, PageChangeKind},
};

/// Most sub-pages one snapshot makes.
const MAX_NEW_PAGES: usize = 50;

/// Applies a Notion snapshot to the page linked to it.
pub struct ApplyNotionSnapshot;

impl EventHandler for ApplyNotionSnapshot {
    type Event = NotionPageRead;
    const NAME: &'static str = "pages.apply_notion_snapshot";
    const QUEUE: &'static str = "default";

    #[allow(
        clippy::too_many_lines,
        reason = "one transaction read top to bottom: check, sub-pages, merge, store, keep"
    )]
    async fn handle(ctx: &mut JobContext, event: Event<NotionPageRead>) -> Result<(), JobError> {
        let read = event.payload;
        let workspace: WorkspaceId = read
            .workspace
            .parse()
            .map_err(|_| JobError::fatal("notion_page_read names no workspace"))?;
        let state = ctx.state().clone();
        let tx = ctx.tx().await?;
        let row: Option<PageRow> = sqlx::query_as(SELECT_LOCKED)
            .bind(read.page)
            .fetch_optional(&mut **tx)
            .await?;
        let Some(row) = row.filter(|row| !row.is_trashed()) else {
            return Ok(());
        };
        let Some(link) = notion::lock_link(tx, read.page).await? else {
            return Ok(());
        };
        if !link.syncing()
            || link.connection_id != read.connection
            || link.notion_page_id != read.notion_page
            || link.last_read_at.is_some_and(|at| at >= read.read_at)
        {
            return Ok(());
        }
        let Some(snapshot) = read_snapshot(&state, workspace, &read.snapshot).await? else {
            tracing::warn!(%workspace, "a Notion snapshot could not be read; it is not applied");
            return Ok(());
        };
        if snapshot.version != 1 || snapshot.notion_page != read.notion_page {
            tracing::warn!(%workspace, "a Notion snapshot is not the page's; it is not applied");
            return Ok(());
        }
        let made = sub_pages(&state, tx, workspace, &row, &link, &snapshot).await?;
        let resolver = notion::resolver(tx, &state, workspace, read.page).await?;
        let mappings = notion::mappings(tx, read.page).await?;
        let loaded = store::load(tx, read.page).await?;
        let merged = match merge_into(&loaded.doc, &snapshot, &resolver, mappings) {
            Ok(merged) => merged,
            Err(rejection) => {
                tracing::warn!(%workspace, code = rejection.code(), "a Notion snapshot would make a body the schema refuses");
                return Err(JobError::fatal(
                    "a Notion snapshot would make a body the schema refuses",
                ));
            }
        };
        let access = Access {
            workspace,
            user: Some(link.created_by),
            role: None,
        };
        hold_files(
            &state,
            tx,
            workspace,
            read.page,
            &link,
            &merged.outcome.files,
        )
        .await?;
        if let Some(update) = &merged.update {
            body::store(tx, &access, read.page, &loaded, update, &merged.inspection).await?;
            announce(&state, tx, &access, read.page, vec![PageChangeKind::Body]).await?;
        }
        notion::write_mappings(
            tx,
            workspace,
            read.page,
            &merged.outcome.upserts,
            &merged.outcome.deletes,
        )
        .await?;
        let title = keep_title(tx, &state, &access, &row, &link, &snapshot).await?;
        sqlx::query(
            "UPDATE pages.external_links
                SET last_inbound_at = now(), last_read_at = $2, conflicts = $3, last_title = $4
              WHERE page_id = $1",
        )
        .bind(read.page)
        .bind(read.read_at)
        .bind(i32::try_from(merged.conflicts).unwrap_or(i32::MAX))
        .bind(title)
        .execute(&mut **tx)
        .await?;
        let moved = merged.outcome.touched + made;
        if moved > 0 {
            tracked::notion_applied(
                &state,
                tx,
                workspace,
                read.page,
                tracked::Direction::Inbound,
                moved,
            )
            .await?;
        }
        if merged.outcome.raised > 0 {
            tracked::notion_conflict_raised(
                &state,
                tx,
                workspace,
                read.page,
                merged.outcome.raised,
            )
            .await?;
        }
        if merged.outcome.ours_pending || merged.ids_given > 0 {
            notion::schedule_push(tx, workspace, read.page, chrono::Utc::now()).await?;
        }
        tracing::info!(
            %workspace,
            blocks = merged.outcome.touched,
            conflicts = merged.conflicts,
            pages = made,
            "a Notion snapshot was applied"
        );
        Ok(())
    }
}

/// The snapshot `blob` holds in `workspace`, when it is at most
/// [`MAX_SNAPSHOT_BYTES`] and parses.
async fn read_snapshot(
    state: &AppState,
    workspace: WorkspaceId,
    blob: &BlobRef,
) -> Result<Option<Snapshot>, JobError> {
    if usize::try_from(blob.size).unwrap_or(usize::MAX) > MAX_SNAPSHOT_BYTES {
        return Ok(None);
    }
    let mut stream = state.blobs.get(&workspace.to_string(), blob).await?;
    let mut bytes: Vec<u8> = Vec::new();
    while let Some(chunk) = stream.next().await {
        bytes.extend_from_slice(&chunk?);
        if bytes.len() > MAX_SNAPSHOT_BYTES {
            return Ok(None);
        }
    }
    Ok(serde_json::from_slice(&bytes).ok())
}

/// The sub-pages `blocks` name, with their Notion pages and titles.
fn sub_page_blocks(blocks: &[crate::domain::notion::model::Block], out: &mut Vec<(Uuid, String)>) {
    for block in blocks {
        if block.kind == Kind::PageLink
            && let Some(page) = block.page
            && !out.iter().any(|(known, _)| *known == page)
        {
            out.push((page, block.text()));
        }
        sub_page_blocks(&block.children, out);
    }
}

/// Make the sub-pages the snapshot names that are not pages here yet, each
/// linked to its Notion page. How many were made.
async fn sub_pages(
    state: &AppState,
    tx: &mut Transaction<'static, Postgres>,
    workspace: WorkspaceId,
    parent: &PageRow,
    link: &Link,
    snapshot: &Snapshot,
) -> Result<usize, JobError> {
    let mut wanted = Vec::new();
    sub_page_blocks(&snapshot.blocks, &mut wanted);
    if wanted.is_empty() {
        return Ok(0);
    }
    let ids: Vec<Uuid> = wanted.iter().map(|(page, _)| *page).collect();
    let known: Vec<Uuid> = sqlx::query_scalar(
        "SELECT notion_page_id FROM pages.external_links
          WHERE connection_id = $1 AND notion_page_id = ANY($2) AND state <> 'stopped'",
    )
    .bind(link.connection_id)
    .bind(&ids)
    .fetch_all(&mut **tx)
    .await?;
    let above = roles::chain(tx, parent.id).await?;
    if above.len() >= MAX_DEPTH {
        return Ok(0);
    }
    let place: Place = parent.place()?;
    let access = Access {
        workspace,
        user: Some(link.created_by),
        role: None,
    };
    let mut made = 0;
    for (notion_page, title) in wanted
        .into_iter()
        .filter(|(page, _)| !known.contains(page))
        .take(MAX_NEW_PAGES)
    {
        let title = clean_title(&title.chars().take(200).collect::<String>()).unwrap_or_default();
        make_sub_page(state, tx, &access, parent, link, notion_page, &title, place).await?;
        made += 1;
    }
    Ok(made)
}

/// One sub-page of `parent`, linked to `notion_page`.
#[allow(
    clippy::too_many_arguments,
    reason = "one call site; the values are distinct"
)]
async fn make_sub_page(
    state: &AppState,
    tx: &mut Transaction<'static, Postgres>,
    access: &Access,
    parent: &PageRow,
    link: &Link,
    notion_page: Uuid,
    title: &str,
    place: Place,
) -> Result<(), JobError> {
    let id = PageId::new();
    let key = position::next(tx, Some(parent.id), place, link.created_by).await?;
    sqlx::query(
        "INSERT INTO pages.pages
             (id, workspace_id, parent_id, place, position, title, created_by, edited_by)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $7)",
    )
    .bind(id)
    .bind(access.workspace.as_uuid())
    .bind(parent.id)
    .bind(place.as_str())
    .bind(&key)
    .bind(title)
    .bind(link.created_by)
    .execute(&mut **tx)
    .await?;
    sqlx::query(
        "INSERT INTO pages.external_links
             (page_id, workspace_id, connection_id, notion_page_id, state, created_by,
              last_title)
         VALUES ($1, $2, $3, $4, 'syncing', $5, $6)",
    )
    .bind(id)
    .bind(access.workspace.as_uuid())
    .bind(link.connection_id)
    .bind(notion_page)
    .bind(link.created_by)
    .bind(title)
    .execute(&mut **tx)
    .await?;
    state
        .events
        .publish(
            tx,
            &NotionLinkRequested {
                workspace: access.workspace.to_string(),
                page: id,
                connection: link.connection_id,
                notion_page,
            },
        )
        .await?;
    announce(state, tx, access, id, vec![PageChangeKind::Meta]).await?;
    tracked::simple(state, tx, access.workspace, id, tracked::Change::Created).await?;
    Ok(())
}

/// The page holds the files its new blocks name.
async fn hold_files(
    state: &AppState,
    tx: &mut Transaction<'static, Postgres>,
    workspace: WorkspaceId,
    page: PageId,
    link: &Link,
    files: &[FileRef],
) -> Result<(), JobError> {
    let key = workspace.to_string();
    let mut seen: Vec<&str> = Vec::new();
    for file in files {
        if seen.contains(&file.blob.as_str()) {
            continue;
        }
        seen.push(&file.blob);
        let Ok(hash) = file.blob.parse::<BlobHash>() else {
            continue;
        };
        let size = i64::try_from(file.size).unwrap_or(i64::MAX);
        sqlx::query(
            "INSERT INTO pages.files (page_id, workspace_id, hash, size, content_type, uploaded_by)
             VALUES ($1, $2, $3, $4, $5, $6) ON CONFLICT DO NOTHING",
        )
        .bind(page)
        .bind(workspace.as_uuid())
        .bind(&file.blob)
        .bind(size)
        .bind(&file.content_type)
        .bind(link.created_by)
        .execute(&mut **tx)
        .await?;
        let blob = BlobRef {
            hash,
            size: file.size,
            content_type: file.content_type.clone(),
        };
        state
            .blobs
            .attach(
                tx,
                &key,
                &blob,
                &BlobOwner::new("pages.page", *page.as_uuid()),
            )
            .await?;
    }
    Ok(())
}

/// Keep Notion's title: the page takes it when it has not been renamed here
/// since the last read. The title kept as Notion's now.
async fn keep_title(
    tx: &mut Transaction<'static, Postgres>,
    state: &AppState,
    access: &Access,
    row: &PageRow,
    link: &Link,
    snapshot: &Snapshot,
) -> Result<String, JobError> {
    let theirs =
        clean_title(&snapshot.title.chars().take(200).collect::<String>()).unwrap_or_default();
    let seen = link.last_title.clone().unwrap_or_default();
    if theirs != seen && row.title == seen && theirs != row.title {
        sqlx::query(
            "UPDATE pages.pages
                SET title = $2, meta_revision = meta_revision + 1, revision = revision + 1
              WHERE id = $1",
        )
        .bind(row.id)
        .bind(&theirs)
        .execute(&mut **tx)
        .await?;
        announce(state, tx, access, row.id, vec![PageChangeKind::Meta]).await?;
    }
    Ok(theirs)
}
