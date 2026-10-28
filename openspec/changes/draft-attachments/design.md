## Context

A draft's `attachments` is a JSON column of `StoredAttachment`
(`id, filename, content_type, size, blob`). `create_draft` fills it for a
forward and attaches each blob to the owner `conversations.draft/<id>`, so
the sweeper keeps the blobs. `update_draft` takes a keep-list of ids.
`release_send` writes every stored attachment's blob reference into the
outgoing document. An uploaded file therefore only needs to become one more
`StoredAttachment` whose blob the draft holds.

## Decisions

### 1. The operation

`POST /api/v1/w/{workspace_id}/drafts/{draft_id}/attachments?version=N&filename=…`
takes the raw bytes as the body (as `pages.upload_file` does), with the
`X-Content-Token` header carrying the draft's `content_token`.

- **Scope:** `conversations:send`, plus the `sender` or `manager` role on the
  draft's inbox.
- **Who:** the draft's author only; anyone else gets 404, as for every draft
  operation.
- **Assistant:** not marked `.assistant()`, so Scout can't call it.
- **Risk:** `Risk::Low`. It records `conversations.draft` with write access.
- **Answer:** 201 with the `DraftView` (new version and token).

**Errors:**

| Status | Code | When |
|---|---|---|
| 400 | `file_empty` | No bytes |
| 400 | `filename_missing` | No usable name after cleaning |
| 400 | `attachment_blocked` | A blocked extension (§3) |
| 409 | `draft_changed` / `content_changed` | As a save |
| 409 | `draft_not_editable` | Scheduled or sending |
| 413 | `attachments_too_large` | The file alone, or the draft's total, over 25 MiB |
| 422 | `too_many_attachments` | Over 20 files |

The route sets `DefaultBodyLimit::max(MAX_ATTACHMENT_BYTES)` (25 MiB). The
kernel blob store's 50 MiB object cap stays the outer bound.

**Order of work:**

1. Read the bytes.
2. Begin a transaction and lock the draft row (`FOR UPDATE`).
3. Check the version, token, state and limits.
4. `blobs.put`.
5. Append the `StoredAttachment` with a fresh `AttachmentId` (UUIDv7) and
   `source: uploaded`.
6. Attach the blob to `conversations.draft/<id>`.
7. Bump the version and commit.

A blob written before a refused commit is unreferenced, and the sweeper
removes it. The blob is written after the lock and checks, so a refused
upload rarely leaves one behind.

### 2. `source`

`DraftAttachment` and `StoredAttachment` gain
`source: "forwarded" | "uploaded"`. It is `#[serde(default)]` →
`forwarded`, so stored rows and old clients keep working, and there is no
migration. The keep-list in `update_draft` drops either kind. A dropped
uploaded blob is detached from the draft, so the sweeper can collect it.
Forwarded blobs are also held by their message, so detaching them changes
nothing.

### 3. Names and blocked types

- **Filename cleaning:**
  - take the part after the last `/` or `\`;
  - drop control characters and `"`;
  - trim spaces and dots at both ends;
  - cut to 255 bytes on a char boundary, keeping the extension when
    possible.
  - A name left empty is refused with `filename_missing`.
- **Blocked extensions** (Gmail's list, case-insensitive): ade, adp, apk,
  appx, appxbundle, bat, cab, chm, cmd, com, cpl, diagcab, diagcfg,
  diagpack, dll, dmg, ex, ex_, exe, hta, img, ins, iso, isp, jar, jnlp, js,
  jse, lib, lnk, mde, msc, msi, msix, msixbundle, msp, mst, nsh, pif, ps1,
  scr, sct, shb, sys, vb, vbe, vbs, vhd, vxd, wsc, wsf, wsh, xll. The list
  is one const in `domain/attachments.rs`, with a test.
- **Content type:** the request's `Content-Type`, lower-cased and checked
  against a `type/subtype` token grammar. If it fails the check it becomes
  `application/octet-stream`. It is never used to decide anything.

### 4. Client

- `uploadAttachment(workspaceId, draft, file)` posts the `File` as the body
  with `Content-Type: file.type || application/octet-stream`.
- `useDraftEditor` gets `attach(files)`, which queues behind any save in
  flight. Each upload uses the version and token the last answer gave, and
  its answer becomes the draft shown. It handles a 409 the same way a save
  does.
- `MessageComposer` gains:
  - an Attach icon button in the footer (`compose.attach` ⇧⌘A), with a
    hidden `<input type=file multiple>`;
  - a drop zone over the composer while files are dragged in (only for
    `Files` drags);
  - paste: `clipboardData.files` attach, and text pastes as before;
  - chips with a file-type tile, the name (ellipsised), the size, Remove,
    and "Uploading…" while pending;
  - a total line, "3.2 MB of 25 MB", shown once anything is attached.
- **Before uploading,** the client refuses a file that would pass 25 MB in
  total, an empty file, or a blocked extension, with a toast naming the file
  and why. The server checks again.

## Data, events, scopes, AI

- **Data:** blobs in the workspace's blob store, held by the draft
  (`kernel.blob_refs`). The draft's JSON gains `source`. No migration.
- **Events:** none new. The operation is a command on `conversations.draft`
  and is recorded like `update_draft`.
- **Scope:** `conversations:send`.
- **AI:** none.
- **Tracked events (ADR 0020):** none. Add the Coverage row for the new
  operation `conversations.upload_draft_attachment`, as the tracking plan
  requires for every operation.

## Risks

- **Large uploads hold a request open.** 25 MiB is the same order as the
  Pages upload (20 MiB), and the same limits apply.
- **Abuse:** at most 20 files and 25 MiB per draft. Deleting or sending a
  draft must release its uploaded blobs, the way its forwarded ones are
  released. Check that the delete and sent paths detach the draft's owner
  refs, and add a test.
