//! `GET` and `PUT …/person-notes`, `DELETE …/person-notes/{note_id}` —
//! what a member told Scout about a person (campbooks-cloud#272): who they
//! are and how to write to them.
//!
//! A note is its member's alone: every statement is keyed by the caller's
//! own id, and nobody else in the workspace, owners and admins included,
//! lists, changes or deletes it (404). It is keyed by the person's address,
//! which travels only in request and response bodies, never in a URL. The
//! only thing that reads it besides these operations is Scout writing that
//! same member a reply to that person (`conversations.write_reply`).
//!
//! Notes never go into events, job payloads, errors or logs: what is logged
//! is a count.

#[cfg(test)]
mod tests;

use std::collections::HashMap;

use axum::{Json, extract::State, http::StatusCode};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, RecordRef, Result, Risk,
    authz::{self, Action},
    db::BeginFor as _,
    extract,
    tenancy::{Access, UserId, Workspace, WorkspaceId},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgConnection;
use utoipa::ToSchema;

use crate::{
    domain::{
        access::visible_inboxes,
        person::{kept_char, plain_address},
        resources::PERSON_NOTE_RECORD,
    },
    model::{InboxId, PersonNoteId},
    scopes::{ConversationsRead, ConversationsWrite},
};

/// Longest note kept, in characters.
pub const MAX_NOTE_CHARS: usize = 500;
/// Most notes one member keeps in a workspace.
pub const MAX_NOTES: i64 = 500;
/// Longest address taken, in characters (as the mail's own).
const MAX_ADDRESS_CHARS: usize = 320;
/// Longest name shown with a note, in characters.
const MAX_NAME_CHARS: usize = 200;

/// The caller's notes, newest change first.
const LIST: &str = "SELECT id, address, note, updated_at FROM conversations.person_notes \
     WHERE workspace_id = $1 AND user_id = $2 \
     ORDER BY updated_at DESC, id DESC LIMIT $3";
/// The name each address last gave in mail it sent to an inbox the caller
/// reads, spam left out. Served by `messages_inbound_sender_idx`.
const NAMES: &str = "SELECT DISTINCT ON (sender_address) sender_address, \
            btrim(sender->>'name') \
       FROM conversations.messages \
      WHERE inbox_id = ANY($1) AND direction = 'inbound' AND sender_address = ANY($2) \
        AND NOT in_spam AND nullif(btrim(sender->>'name'), '') IS NOT NULL \
      ORDER BY sender_address, sent_at DESC";
/// One writer at a time per member, so the limit holds.
const LOCK: &str = "SELECT pg_advisory_xact_lock(hashtextextended($1, 0))";
/// Whether the caller already has a note for the address, and how many
/// they have.
const COUNT: &str = "SELECT count(*), coalesce(bool_or(address = $3), false) \
       FROM conversations.person_notes WHERE workspace_id = $1 AND user_id = $2";
/// Keep the note, a new one or the caller's own changed.
const UPSERT: &str = "INSERT INTO conversations.person_notes \
         (id, workspace_id, user_id, address, note) VALUES ($1, $2, $3, $4, $5) \
     ON CONFLICT (workspace_id, user_id, address) DO UPDATE \
        SET note = EXCLUDED.note, updated_at = now() \
     RETURNING id, address, note, updated_at";
/// Delete the caller's own note.
const DELETE: &str = "DELETE FROM conversations.person_notes \
      WHERE id = $1 AND workspace_id = $2 AND user_id = $3";
/// The caller's note about one address.
const ABOUT: &str = "SELECT note FROM conversations.person_notes \
      WHERE workspace_id = $1 AND user_id = $2 AND address = $3";

/// A row of [`LIST`] and [`UPSERT`].
type NoteRow = (PersonNoteId, String, String, DateTime<Utc>);

/// `…/person-notes/{note_id}`
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct NotePath {
    /// The note.
    pub note_id: PersonNoteId,
}

/// What you told Scout about a person.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct PersonNote {
    /// The note.
    pub id: PersonNoteId,
    /// The person's address, lower-cased.
    pub address: String,
    /// The name they gave in mail to an inbox you read, when there is one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Who they are and how to write to them, in your words.
    pub note: String,
    /// When you last changed it.
    pub updated_at: DateTime<Utc>,
}

/// Your notes, the newest change first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct PersonNoteList {
    /// At most 500.
    pub notes: Vec<PersonNote>,
}

/// The note to keep about a person.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SetPersonNote {
    /// The person's address.
    #[schema(example = "marta@clientes.example")]
    pub address: String,
    /// Who they are and how to write to them (1 to 500 characters).
    #[schema(example = "My landlord. Formal, in Portuguese.")]
    pub note: String,
}

/// List what you told Scout about people.
#[utoipa::path(
    get,
    path = "/api/v1/w/{workspace_id}/person-notes",
    operation_id = "conversations.list_person_notes",
    tag = "conversations",
    security(("bearer" = ["conversations:read"])),
    extensions(("x-campbooks" = json!(Operation::query().records(["conversations.person_note"]).access("conversations.person_note", campbooks_kernel::authz::Action::Read)))),
    params(("workspace_id" = WorkspaceId, Path, description = "The workspace.")),
    responses(
        (status = OK, description = "Your notes; nobody else's.", body = PersonNoteList),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, or not a member (`person_notes_need_member`).",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such workspace, or you are not a member.",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn list(
    State(state): State<AppState>,
    workspace: Workspace<ConversationsRead>,
) -> Result<Json<PersonNoteList>> {
    let access = &workspace.access;
    let user = own_notes(&state, access, Action::Read)?;
    let mut tx = state.db.begin_for(access.workspace).await?;
    let rows: Vec<NoteRow> = sqlx::query_as(LIST)
        .bind(access.workspace.as_uuid())
        .bind(user.as_uuid())
        .bind(MAX_NOTES)
        .fetch_all(&mut *tx)
        .await?;
    let addresses: Vec<String> = rows.iter().map(|row| row.1.clone()).collect();
    let mut names = names_of(&mut tx, access, &addresses).await?;
    tx.commit().await?;
    let notes = rows
        .into_iter()
        .map(|(id, address, note, updated_at)| PersonNote {
            id,
            name: names.remove(&address),
            address,
            note,
            updated_at,
        })
        .collect();
    Ok(Json(PersonNoteList { notes }))
}

/// Tell Scout about a person, or change what you told it.
#[utoipa::path(
    put,
    path = "/api/v1/w/{workspace_id}/person-notes",
    operation_id = "conversations.set_person_note",
    tag = "conversations",
    security(("bearer" = ["conversations:write"])),
    // Undo of a change is another one with the note it replaced; of a new
    // note, `conversations.delete_person_note`.
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .records(["conversations.person_note"])
        .access("conversations.person_note", campbooks_kernel::authz::Action::Write)))),
    params(("workspace_id" = WorkspaceId, Path, description = "The workspace.")),
    request_body = SetPersonNote,
    responses(
        (status = OK, description = "The note as kept.", body = PersonNote),
        (status = BAD_REQUEST, description = "Not a plain address, or a note that is empty or too long (`person_note_invalid`).",
            body = Problem, content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, or not a member (`person_notes_need_member`).",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such workspace, or you are not a member.",
            body = Problem, content_type = "application/problem+json"),
        (status = CONFLICT, description = "You keep 500 notes already (`person_notes_full`): remove one first.",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn set(
    State(state): State<AppState>,
    workspace: Workspace<ConversationsWrite>,
    extract::Json(body): extract::Json<SetPersonNote>,
) -> Result<Json<PersonNote>> {
    let access = &workspace.access;
    let user = own_notes(&state, access, Action::Write)?;
    let (Some(address), Some(note)) = (kept_address(&body.address), kept_note(&body.note)) else {
        return Err(Error::BadRequest(
            crate::MESSAGES
                .reason("person_note_invalid")
                .arg("max", MAX_NOTE_CHARS),
        ));
    };
    let mut tx = state.db.begin_for(access.workspace).await?;
    sqlx::query(LOCK)
        .bind(format!(
            "conversations.person_notes:{}:{user}",
            access.workspace
        ))
        .execute(&mut *tx)
        .await?;
    let (count, known): (i64, bool) = sqlx::query_as(COUNT)
        .bind(access.workspace.as_uuid())
        .bind(user.as_uuid())
        .bind(&address)
        .fetch_one(&mut *tx)
        .await?;
    if !known && count >= MAX_NOTES {
        return Err(Error::Conflict(
            crate::MESSAGES
                .reason("person_notes_full")
                .arg("max", MAX_NOTES),
        ));
    }
    let (id, address, note, updated_at): NoteRow = sqlx::query_as(UPSERT)
        .bind(PersonNoteId::new())
        .bind(access.workspace.as_uuid())
        .bind(user.as_uuid())
        .bind(&address)
        .bind(&note)
        .fetch_one(&mut *tx)
        .await?;
    let mut names = names_of(&mut tx, access, std::slice::from_ref(&address)).await?;
    tx.commit().await?;
    tracing::info!(new = !known, "a person note kept");
    Ok(Json(PersonNote {
        id,
        name: names.remove(&address),
        address,
        note,
        updated_at,
    }))
}

/// Forget what you told Scout about a person.
#[utoipa::path(
    delete,
    path = "/api/v1/w/{workspace_id}/person-notes/{note_id}",
    operation_id = "conversations.delete_person_note",
    tag = "conversations",
    security(("bearer" = ["conversations:write"])),
    // Undo keeps the same note again.
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .undo("conversations.set_person_note")
        .records(["conversations.person_note"])
        .access("conversations.person_note", campbooks_kernel::authz::Action::Delete)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("note_id" = PersonNoteId, Path, description = "The note."),
    ),
    responses(
        (status = NO_CONTENT, description = "Forgotten."),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope.", body = Problem,
            content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such note of yours.", body = Problem,
            content_type = "application/problem+json"),
    )
)]
pub async fn delete(
    State(state): State<AppState>,
    workspace: Workspace<ConversationsWrite>,
    extract::Path(path): extract::Path<NotePath>,
) -> Result<StatusCode> {
    let access = &workspace.access;
    let mut tx = state.db.begin_for(access.workspace).await?;
    // 404 unless it is the caller's own.
    authz::require(
        &state,
        &mut tx,
        access,
        Action::Delete,
        &RecordRef::new(PERSON_NOTE_RECORD, &path.note_id),
        || Error::NotFound,
    )
    .await?;
    let user = access.user.ok_or(Error::NotFound)?;
    let deleted = sqlx::query(DELETE)
        .bind(path.note_id)
        .bind(access.workspace.as_uuid())
        .bind(user.as_uuid())
        .execute(&mut *tx)
        .await?
        .rows_affected();
    if deleted == 0 {
        return Err(Error::NotFound);
    }
    tx.commit().await?;
    tracing::info!("a person note forgotten");
    Ok(StatusCode::NO_CONTENT)
}

/// The caller, when they may take `action` on their own notes: a member,
/// or an app acting as one. A client acting for nobody has no notes:
/// 403 `person_notes_need_member`.
fn own_notes(state: &AppState, access: &Access, action: Action) -> Result<UserId> {
    authz::require_type(state, access, action, PERSON_NOTE_RECORD, needs_member)?;
    access.user.ok_or_else(needs_member)
}

fn needs_member() -> Error {
    Error::Denied(crate::MESSAGES.reason("person_notes_need_member"))
}

/// The address as kept: trimmed, lower-cased, plain and at most
/// [`MAX_ADDRESS_CHARS`] characters.
fn kept_address(address: &str) -> Option<String> {
    let address = address.trim().to_lowercase();
    let length = address.chars().count();
    ((3..=MAX_ADDRESS_CHARS).contains(&length) && plain_address(&address)).then_some(address)
}

/// The note as kept: line breaks normalised, no control characters but line
/// breaks, no invisible direction or zero-width marks, trimmed, 1 to
/// [`MAX_NOTE_CHARS`] characters.
fn kept_note(note: &str) -> Option<String> {
    let note: String = note
        .replace("\r\n", "\n")
        .chars()
        .filter(|c| *c != '\t' && kept_char(*c))
        .collect();
    let note = note.trim();
    let length = note.chars().count();
    (1..=MAX_NOTE_CHARS)
        .contains(&length)
        .then(|| note.to_owned())
}

/// The name each of `addresses` gave in mail to an inbox `access` reads.
async fn names_of(
    conn: &mut PgConnection,
    access: &Access,
    addresses: &[String],
) -> Result<HashMap<String, String>> {
    if addresses.is_empty() {
        return Ok(HashMap::new());
    }
    let inboxes: Vec<InboxId> = visible_inboxes(conn, access).await?;
    if inboxes.is_empty() {
        return Ok(HashMap::new());
    }
    let rows: Vec<(String, String)> = sqlx::query_as(NAMES)
        .bind(inboxes.as_slice())
        .bind(addresses)
        .fetch_all(&mut *conn)
        .await?;
    Ok(rows
        .into_iter()
        .map(|(address, name)| (address, name.chars().take(MAX_NAME_CHARS).collect()))
        .collect())
}

/// What `user` told Scout about `address` (lower-cased), for a reply Scout
/// writes them: their own note only, never a teammate's.
pub async fn note_about(
    conn: &mut PgConnection,
    workspace: WorkspaceId,
    user: UserId,
    address: &str,
) -> Result<Option<String>> {
    Ok(sqlx::query_scalar::<_, String>(ABOUT)
        .bind(workspace.as_uuid())
        .bind(user.as_uuid())
        .bind(address)
        .fetch_optional(conn)
        .await?)
}
