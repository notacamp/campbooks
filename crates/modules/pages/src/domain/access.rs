//! The one door to a page: ask the central check (ADR 0017), then load the
//! row, locked when the caller is about to change it.

use campbooks_kernel::{
    AppState, Error, RecordRef, Result,
    authz::{self, Action},
    tenancy::Access,
};
use sqlx::{Postgres, Transaction};

use super::{PageId, PageRow, SELECT_LOCKED, SELECT_ONE, resources::PAGE_RECORD};

/// Page `id` as `access` may take `action` on it, or 404 when they may not
/// see it (403 `page_forbidden` when they see it but may not do that).
/// `lock` holds the row for the rest of the transaction.
pub async fn require_page(
    state: &AppState,
    tx: &mut Transaction<'_, Postgres>,
    access: &Access,
    action: Action,
    id: PageId,
    lock: bool,
) -> Result<PageRow> {
    authz::require(
        state,
        tx,
        access,
        action,
        &RecordRef::new(PAGE_RECORD, &id),
        || Error::Denied(crate::MESSAGES.reason("page_forbidden")),
    )
    .await?;
    sqlx::query_as::<_, PageRow>(if lock { SELECT_LOCKED } else { SELECT_ONE })
        .bind(id)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(Error::NotFound)
}
