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
use serde_json::Value;
use sqlx::PgConnection;
use utoipa::ToSchema;

use crate::{
    domain::{
        access::visible_inboxes,
        person::{distrusted, kept_char, plain_address},
        resources::PERSON_NOTE_RECORD,
    },
    model::{InboxId, PersonNoteId},
    scopes::{ConversationsRead, ConversationsWrite},
    tracking,
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
/// Sent messages of each inbox looked through for the name it gave a
/// person. Served by `messages_outbound_idx`.
const SENT_SCAN: i64 = 500;
/// Received messages per inbox and address looked through for the name a
/// person gave. Served by `messages_inbound_sender_idx`.
const RECEIVED_SCAN: i64 = 10;

/// The name the caller's inboxes (`$1`) gave each address (`$2`, lower-cased)
/// when writing to it, as a To or Cc recipient of their newest `$3` sent
/// messages each, newest first: the member's own mail, never the sender's
/// say.
const SENT_NAMES: &str = "SELECT DISTINCT ON (address) address, name FROM ( \
         SELECT lower(r->>'address') AS address, btrim(r->>'name') AS name, \
                s.sent_at, s.id \
           FROM unnest($1::uuid[]) AS i(id) \
           CROSS JOIN LATERAL ( \
               SELECT id, sent_at, recipients FROM conversations.messages \
                WHERE inbox_id = i.id AND direction = 'outbound' \
                ORDER BY sent_at DESC, id DESC LIMIT $3) s \
           CROSS JOIN LATERAL jsonb_array_elements( \
               CASE WHEN jsonb_typeof(s.recipients->'to') = 'array' \
                    THEN s.recipients->'to' ELSE '[]'::jsonb END || \
               CASE WHEN jsonb_typeof(s.recipients->'cc') = 'array' \
                    THEN s.recipients->'cc' ELSE '[]'::jsonb END) r) named \
      WHERE address = ANY($2) AND nullif(name, '') IS NOT NULL \
      ORDER BY address, sent_at DESC, id DESC";
/// Up to `$3` messages each address (`$2`) sent to each of the caller's
/// inboxes (`$1`), with what decides whether they are trusted: in spam,
/// their headers, and whether their conversation is flagged suspicious or
/// has an open Review need (as `conversations.write_reply` reads them), and
/// when they were stored.
const RECEIVED_NAMES: &str = "SELECT a.address, btrim(m.sender->>'name'), m.created_at, \
            m.in_spam, m.headers, \
            (EXISTS (SELECT 1 FROM conversations.decisions d \
                      WHERE d.conversation_id = m.conversation_id \
                        AND d.workspace_id = m.workspace_id \
                        AND d.question = 'suspicious' AND d.value = 'true'::jsonb) \
             OR EXISTS (SELECT 1 FROM conversations.needs n \
                         WHERE n.conversation_id = m.conversation_id \
                           AND n.workspace_id = m.workspace_id \
                           AND n.state = 'open' AND n.reason = 'review')) \
       FROM unnest($1::uuid[]) AS i(id) \
       CROSS JOIN unnest($2::text[]) AS a(address) \
       CROSS JOIN LATERAL ( \
           SELECT conversation_id, workspace_id, sender, created_at, in_spam, headers \
             FROM conversations.messages \
            WHERE inbox_id = i.id AND direction = 'inbound' AND sender_address = a.address \
            ORDER BY sent_at DESC LIMIT $3) m";
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
    /// Their name as your mail gave it, else as their trusted mail did.
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
    // What it said before, to track a change only (never the text itself).
    let before: Option<String> = if known {
        sqlx::query_scalar(ABOUT)
            .bind(access.workspace.as_uuid())
            .bind(user.as_uuid())
            .bind(&address)
            .fetch_optional(&mut *tx)
            .await?
    } else {
        None
    };
    let (id, address, note, updated_at): NoteRow = sqlx::query_as(UPSERT)
        .bind(PersonNoteId::new())
        .bind(access.workspace.as_uuid())
        .bind(user.as_uuid())
        .bind(&address)
        .bind(&note)
        .fetch_one(&mut *tx)
        .await?;
    let mut names = names_of(&mut tx, access, std::slice::from_ref(&address)).await?;
    if before.as_deref() != Some(note.as_str()) {
        state
            .tracker
            .track(
                &mut tx,
                &tracking::PersonNoteSaved {
                    workspace: access.workspace,
                    note_id: *id.as_uuid(),
                    created: !known,
                },
            )
            .await?;
    }
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
    state
        .tracker
        .track(
            &mut tx,
            &tracking::PersonNoteDeleted {
                workspace: access.workspace,
                note_id: *path.note_id.as_uuid(),
            },
        )
        .await?;
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
    // First, the name the member's own mail gave them.
    let sent: Vec<(String, String)> = sqlx::query_as(SENT_NAMES)
        .bind(inboxes.as_slice())
        .bind(addresses)
        .bind(SENT_SCAN)
        .fetch_all(&mut *conn)
        .await?;
    let mut names: HashMap<String, String> = sent
        .into_iter()
        .filter_map(|(address, name)| shown(&name).map(|name| (address, name)))
        .collect();
    let rest: Vec<String> = addresses
        .iter()
        .filter(|address| !names.contains_key(*address))
        .cloned()
        .collect();
    if rest.is_empty() {
        return Ok(names);
    }
    // Else the name they gave in trusted mail, the one stored last: never
    // by `sent_at`, which the sender sets.
    let received: Vec<ReceivedRow> = sqlx::query_as(RECEIVED_NAMES)
        .bind(inboxes.as_slice())
        .bind(rest.as_slice())
        .bind(RECEIVED_SCAN)
        .fetch_all(&mut *conn)
        .await?;
    let mut newest: HashMap<String, (DateTime<Utc>, String)> = HashMap::new();
    for (address, name, stored, in_spam, headers, suspicious) in received {
        let Some(name) = name.as_deref().and_then(shown) else {
            continue;
        };
        if distrusted("inbound", &address, in_spam, &headers, suspicious) {
            continue;
        }
        if newest.get(&address).is_none_or(|(at, _)| stored > *at) {
            newest.insert(address, (stored, name));
        }
    }
    names.extend(
        newest
            .into_iter()
            .map(|(address, (_, name))| (address, name)),
    );
    Ok(names)
}

/// A row of [`RECEIVED_NAMES`].
type ReceivedRow = (String, Option<String>, DateTime<Utc>, bool, Value, bool);

/// A name as shown: one line, no invisible marks, at most
/// [`MAX_NAME_CHARS`] characters; `None` when nothing is left.
fn shown(name: &str) -> Option<String> {
    let name: String = name
        .chars()
        .map(|c| if c.is_whitespace() { ' ' } else { c })
        .filter(|c| kept_char(*c))
        .collect();
    let name: String = name.split_whitespace().collect::<Vec<_>>().join(" ");
    let name: String = name.chars().take(MAX_NAME_CHARS).collect();
    (!name.is_empty()).then_some(name)
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
