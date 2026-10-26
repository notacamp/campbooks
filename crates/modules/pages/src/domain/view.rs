//! A page as `pages.get` and `pages.create` show it.

use std::collections::HashMap;

use campbooks_kernel::{Error, Result, tenancy::Access};
use sqlx::PgConnection;

use super::{
    Crumb, DocumentState, GrantView, PageId, PageRow, PageView, Role, body, document, roles, store,
};

/// `row` as `access` sees it: their role, who it is shared with, the
/// ancestors they may open, and the body. 404 when they hold no role on it.
pub async fn page_view(
    conn: &mut PgConnection,
    access: &Access,
    row: &PageRow,
) -> Result<PageView> {
    let chain = roles::chain(&mut *conn, row.id).await?;
    let caller = roles::Caller::of(access);
    let ids: Vec<PageId> = chain.iter().map(|node| node.id).collect();
    let grants = match caller {
        Some(caller) => roles::grants_among(&mut *conn, caller.user, &ids).await?,
        None => HashMap::new(),
    };
    let role: Role = caller
        .and_then(|caller| roles::role_in_chain(&chain, &grants, caller))
        .ok_or(Error::NotFound)?;

    // The pages above it that the caller may open, outermost first.
    let mut above: Vec<PageId> = Vec::new();
    for at in 1..chain.len() {
        let opens = caller
            .and_then(|caller| roles::role_in_chain(&chain[at..], &grants, caller))
            .is_some();
        if opens {
            above.push(chain[at].id);
        }
    }
    above.reverse();
    let named: Vec<(PageId, String, Option<String>)> =
        sqlx::query_as("SELECT id, title, icon FROM pages.pages WHERE id = ANY($1)")
            .bind(&above)
            .fetch_all(&mut *conn)
            .await?;
    let ancestors: Vec<Crumb> = above
        .iter()
        .filter_map(|id| {
            named
                .iter()
                .find(|(named, ..)| named == id)
                .map(|(id, title, icon)| Crumb {
                    id: *id,
                    title: title.clone(),
                    icon: icon.clone(),
                })
        })
        .collect();

    let shared: Vec<(campbooks_kernel::tenancy::UserId, String)> = sqlx::query_as(
        "SELECT member_id, role FROM pages.grants WHERE page_id = $1 ORDER BY at, member_id",
    )
    .bind(row.id)
    .fetch_all(&mut *conn)
    .await?;
    let grants_view = shared
        .into_iter()
        .map(|(member, role)| {
            Ok(GrantView {
                member,
                role: role.parse()?,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    let loaded = store::load(conn, row.id).await?;
    Ok(PageView {
        id: row.id,
        parent: row.parent_id,
        place: row.place()?,
        position: row.position.clone(),
        title: row.title.clone(),
        icon: row.icon.clone(),
        role,
        created_by: row.created_by,
        created_at: row.created_at,
        edited_by: row.edited_by,
        edited_at: row.edited_at,
        trashed_at: row.trashed_at,
        revision: row.revision,
        meta_revision: row.meta_revision,
        kind: row.kind()?,
        grants: grants_view,
        ancestors,
        document: DocumentState {
            update: body::encode(&document::full_state(&loaded.doc)),
            state_vector: body::encode(&document::state_vector(&loaded.doc)),
        },
    })
}

/// `row` as a tree entry for `access`: their role and whether it has
/// sub-pages. 404 when they hold no role on it.
pub async fn summary_of(
    conn: &mut PgConnection,
    access: &Access,
    row: &PageRow,
) -> Result<super::PageSummary> {
    let role = roles::standing(&mut *conn, access, row.id)
        .await?
        .flatten()
        .ok_or(Error::NotFound)?;
    let has_children: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM pages.pages
          WHERE parent_id = $1 AND trashed_at IS NULL AND kind <> 'row')",
    )
    .bind(row.id)
    .fetch_one(conn)
    .await?;
    row.summary(role, has_children)
}
