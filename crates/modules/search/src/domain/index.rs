//! Keeping the index current: refreshing one group from its source, and
//! starting backfills.

use campbooks_kernel::{
    RecordRef, Result,
    blobs::BlobStore,
    jobs,
    search::{EntryHead, EntryText, Facet, Group, PersonRole, SearchSource, SourceContext, facet},
    tenancy::WorkspaceId,
};
use sqlx::PgConnection;

use super::{
    EntryId, GroupId, language,
    text::{clean, words},
};

/// The shape of what the index stores (configurations, weights, fields), or
/// what a source reads into it. Bump it when that changes: every
/// workspace's next touch rebuilds its index in the background, answering
/// from the old rows meanwhile, and an entry whose stamp changed is read
/// again. 2: mail read from HTML with no `<body>` tag gets its words
/// (campbooks-cloud#218).
pub const INDEX_VERSION: i16 = 2;

/// Group facet set by search itself: some entry carries files.
pub const ATTACHMENT: &str = "attachment";

/// What a refresh did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refreshed {
    /// The group is indexed as it is now; this many entries' text was read.
    Indexed {
        /// Entries whose text was (re)read.
        read: usize,
    },
    /// The group is gone and was removed from the index.
    Removed,
}

/// Bring group `id` of `source` up to date in `workspace`, inside the
/// caller's transaction (`conn`, workspace set). Refreshes of one group run
/// one at a time (a transaction lock), and each reads the source's current
/// state, so any order of deliveries ends in the same index.
pub async fn refresh(
    conn: &mut PgConnection,
    blobs: &BlobStore,
    workspace: WorkspaceId,
    source: &dyn SearchSource,
    id: &str,
) -> Result<Refreshed> {
    let group_type = source.group_type();
    sqlx::query_scalar!(
        "SELECT pg_advisory_xact_lock(hashtextextended('search.group:' || $1, 0))",
        format!("{workspace}:{group_type}:{id}"),
    )
    .fetch_one(&mut *conn)
    .await?;
    let cx = SourceContext {
        conn: &mut *conn,
        blobs,
        workspace,
    };
    let Some(group) = source.group(cx, id).await? else {
        sqlx::query!(
            "DELETE FROM search.groups WHERE workspace_id = $1 AND record_type = $2 AND record_id = $3",
            workspace.as_uuid(),
            group_type,
            id,
        )
        .execute(&mut *conn)
        .await?;
        return Ok(Refreshed::Removed);
    };

    let title = clean(&group.title);
    let (group_id, previous_title) =
        write_group(conn, workspace, group_type, id, &group, &title).await?;
    let read = write_entries(conn, blobs, workspace, source, group_id, &title, &group).await?;
    if previous_title.is_some_and(|previous| previous != title) {
        // The title is part of every entry's vector.
        sqlx::query!(
            "UPDATE search.entries
                SET tsv = search.entry_vector($1, people_words, file_words, body, language)
              WHERE group_id = $2",
            title,
            group_id as GroupId,
        )
        .execute(&mut *conn)
        .await?;
    }
    Ok(Refreshed::Indexed { read })
}

/// Upsert the group's row; returns its id and the title it had before.
async fn write_group(
    conn: &mut PgConnection,
    workspace: WorkspaceId,
    group_type: &str,
    id: &str,
    group: &Group,
    title: &str,
) -> Result<(GroupId, Option<String>)> {
    let mut facets = group.facets.clone();
    let attachment = group.entries.iter().any(
        |entry| matches!(entry.facets.get(facet::ATTACHMENTS), Some(Facet::Count(n)) if *n > 0),
    );
    facets.insert(ATTACHMENT.to_owned(), Facet::Bool(attachment));
    let facets = serde_json::to_value(&facets).map_err(campbooks_kernel::Error::internal)?;
    let previous_title = sqlx::query_scalar!(
        "SELECT title FROM search.groups WHERE workspace_id = $1 AND record_type = $2 AND record_id = $3",
        workspace.as_uuid(),
        group_type,
        id,
    )
    .fetch_optional(&mut *conn)
    .await?;
    let group_id = sqlx::query_scalar!(
        r#"INSERT INTO search.groups (id, workspace_id, record_type, record_id, container_type,
                                      container_id, occurred_at, title, title_folded, keywords,
                                      facets, index_version, refreshed_at)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, lower(unaccent($8)),
                   ARRAY(SELECT lower(unaccent(k)) FROM unnest($9::text[]) k), $10, $11, now())
           ON CONFLICT (workspace_id, record_type, record_id) DO UPDATE
              SET container_type = EXCLUDED.container_type, container_id = EXCLUDED.container_id,
                  occurred_at = EXCLUDED.occurred_at, title = EXCLUDED.title,
                  title_folded = EXCLUDED.title_folded, keywords = EXCLUDED.keywords,
                  facets = EXCLUDED.facets, index_version = EXCLUDED.index_version,
                  refreshed_at = now()
           RETURNING id AS "id: GroupId""#,
        GroupId::new() as GroupId,
        workspace.as_uuid(),
        group_type,
        id,
        group.container.record_type,
        group.container.id,
        group.occurred_at,
        title,
        &group.keywords,
        facets,
        INDEX_VERSION,
    )
    .fetch_one(&mut *conn)
    .await?;

    Ok((group_id, previous_title))
}

/// Bring the group's entries up to date: facets for every entry, text only
/// for entries whose stamp changed, and entries no longer listed removed.
/// Returns how many texts were read.
async fn write_entries(
    conn: &mut PgConnection,
    blobs: &BlobStore,
    workspace: WorkspaceId,
    source: &dyn SearchSource,
    group_id: GroupId,
    title: &str,
    group: &Group,
) -> Result<usize> {
    let ids: Vec<String> = group.entries.iter().map(|e| e.record.id.clone()).collect();
    let stored = sqlx::query!(
        "SELECT record_type, record_id, stamp FROM search.entries
          WHERE workspace_id = $1 AND record_id = ANY($2)",
        workspace.as_uuid(),
        &ids,
    )
    .fetch_all(&mut *conn)
    .await?;
    let unchanged = |head: &EntryHead| {
        stored.iter().any(|row| {
            row.record_type == head.record.record_type
                && row.record_id == head.record.id
                && row.stamp == head.stamp
        })
    };
    let (keep, read): (Vec<&EntryHead>, Vec<&EntryHead>) =
        group.entries.iter().partition(|head| unchanged(head));

    for head in &keep {
        let facets =
            serde_json::to_value(&head.facets).map_err(campbooks_kernel::Error::internal)?;
        sqlx::query!(
            "UPDATE search.entries SET group_id = $1, occurred_at = $2, facets = $3
              WHERE workspace_id = $4 AND record_type = $5 AND record_id = $6",
            group_id as GroupId,
            head.occurred_at,
            facets,
            workspace.as_uuid(),
            head.record.record_type,
            head.record.id,
        )
        .execute(&mut *conn)
        .await?;
    }
    let records: Vec<RecordRef> = read.iter().map(|head| head.record.clone()).collect();
    let texts = if records.is_empty() {
        Vec::new()
    } else {
        let cx = SourceContext {
            conn: &mut *conn,
            blobs,
            workspace,
        };
        source.texts(cx, &records).await?
    };
    for text in &texts {
        let Some(head) = read.iter().find(|head| head.record == text.record) else {
            continue;
        };
        upsert_entry(conn, workspace, group_id, title, head, text).await?;
    }
    sqlx::query!(
        "DELETE FROM search.entries WHERE group_id = $1 AND NOT (record_id = ANY($2))",
        group_id as GroupId,
        &ids,
    )
    .execute(&mut *conn)
    .await?;
    Ok(texts.len())
}

/// Write one entry's text and vector.
async fn upsert_entry(
    conn: &mut PgConnection,
    workspace: WorkspaceId,
    group: GroupId,
    title: &str,
    head: &EntryHead,
    text: &EntryText,
) -> Result<()> {
    let body = clean(&text.text);
    let language = language::detect(&format!("{title}\n{body}"));
    let line = |role: PersonRole| {
        text.people
            .iter()
            .filter(|person| person.role == role)
            .map(|person| {
                clean(&format!(
                    "{} {}",
                    person.name.as_deref().unwrap_or_default(),
                    person.address
                ))
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    let people_words = words(
        &text
            .people
            .iter()
            .map(|p| format!("{} {}", p.name.as_deref().unwrap_or_default(), p.address))
            .collect::<Vec<_>>()
            .join(" "),
    );
    let attachments: Vec<String> = text.attachments.iter().map(|name| clean(name)).collect();
    let file_words = words(&attachments.join(" "));
    let people = serde_json::to_value(&text.people).map_err(campbooks_kernel::Error::internal)?;
    let facets = serde_json::to_value(&head.facets).map_err(campbooks_kernel::Error::internal)?;
    sqlx::query!(
        r#"INSERT INTO search.entries (id, workspace_id, group_id, record_type, record_id,
                                       occurred_at, language, people, people_words, file_words,
                                       from_folded, to_folded, cc_folded, body, attachments,
                                       facets, stamp, tsv)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, lower(unaccent($11)),
                   lower(unaccent($12)), lower(unaccent($13)), $14, $15, $16, $17,
                   search.entry_vector($18, $9, $10, $14, $7))
           ON CONFLICT (workspace_id, record_type, record_id) DO UPDATE
              SET group_id = EXCLUDED.group_id, occurred_at = EXCLUDED.occurred_at,
                  language = EXCLUDED.language, people = EXCLUDED.people,
                  people_words = EXCLUDED.people_words, file_words = EXCLUDED.file_words,
                  from_folded = EXCLUDED.from_folded, to_folded = EXCLUDED.to_folded,
                  cc_folded = EXCLUDED.cc_folded, body = EXCLUDED.body,
                  attachments = EXCLUDED.attachments, facets = EXCLUDED.facets,
                  stamp = EXCLUDED.stamp, tsv = EXCLUDED.tsv"#,
        EntryId::new() as EntryId,
        workspace.as_uuid(),
        group as GroupId,
        head.record.record_type,
        head.record.id,
        head.occurred_at,
        language.as_str(),
        people,
        people_words,
        file_words,
        line(PersonRole::From),
        line(PersonRole::To),
        line(PersonRole::Cc),
        body,
        &attachments,
        facets,
        head.stamp,
        title,
    )
    .execute(&mut *conn)
    .await?;
    Ok(())
}

/// Where a workspace's index of one source stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Standing {
    /// Built (or being built) at the current [`INDEX_VERSION`].
    Current,
    /// A backfill was just started (its first page enqueued).
    Started,
    /// The workspace is being deleted: index nothing more for it.
    Deleting,
}

/// Start the backfill of `group_type` that `workspace` still needs, if any,
/// and say where its index stands. A plain read answers the common case
/// (nothing to do) without locking anything, so searches and event
/// deliveries never wait on a backfill page; only a missing or outdated row
/// is written, by a statement that cannot wait on a running page either.
pub async fn ensure_backfill(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    workspace: WorkspaceId,
    group_type: &str,
) -> Result<Standing> {
    let row = sqlx::query!(
        "SELECT state, index_version FROM search.backfills
          WHERE workspace_id = $1 AND group_type = $2",
        workspace.as_uuid(),
        group_type,
    )
    .fetch_optional(&mut **tx)
    .await?;
    let started = match row {
        Some(row) if row.state == "deleting" => return Ok(Standing::Deleting),
        Some(row) if row.index_version >= INDEX_VERSION => return Ok(Standing::Current),
        None => sqlx::query_scalar!(
            r#"INSERT INTO search.backfills (workspace_id, group_type, state, index_version)
               VALUES ($1, $2, 'running', $3)
               ON CONFLICT (workspace_id, group_type) DO NOTHING
               RETURNING group_type"#,
            workspace.as_uuid(),
            group_type,
            INDEX_VERSION,
        )
        .fetch_optional(&mut **tx)
        .await?
        .is_some(),
        Some(_) => sqlx::query_scalar!(
            r#"UPDATE search.backfills
                  SET state = 'running', after_id = NULL, index_version = $3,
                      started_at = now(), finished_at = NULL
                WHERE workspace_id = $1 AND group_type = $2 AND index_version < $3
                  AND state <> 'deleting'
               RETURNING group_type"#,
            workspace.as_uuid(),
            group_type,
            INDEX_VERSION,
        )
        .fetch_optional(&mut **tx)
        .await?
        .is_some(),
    };
    if !started {
        return Ok(Standing::Current);
    }
    let job = crate::features::backfill::Backfill {
        group_type: group_type.to_owned(),
        after: None,
    };
    jobs::enqueue(&mut *tx, &job, job.options(workspace)).await?;
    Ok(Standing::Started)
}

/// Whether a backfill of any of `group_types` is still running.
pub async fn indexing(
    conn: &mut PgConnection,
    workspace: WorkspaceId,
    group_types: &[String],
) -> Result<bool> {
    Ok(sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM search.backfills
                           WHERE workspace_id = $1 AND group_type = ANY($2)
                             AND state = 'running') AS "running!""#,
        workspace.as_uuid(),
        group_types,
    )
    .fetch_one(conn)
    .await?)
}

#[cfg(test)]
mod tests {
    use crate::domain::language::Language;

    #[test]
    fn configurations_are_named_after_the_column_values() {
        let names: Vec<_> = Language::ALL.iter().map(|l| l.as_str()).collect();
        assert_eq!(names, ["en", "pt", "es", "fr", "simple"]);
    }
}
