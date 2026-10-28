## Why

The composer can't attach a file from your device. A draft only ever holds
the attachments of a message it forwards, and you can only remove those.

This is phase 2 of the composer rework, from the approved proposal
(https://claude.ai/artifact/BxLcKwjxF6gHbpfiWCeW8M).

Tracking: https://github.com/notacamp/campbooks-cloud/issues/391 (epic #389).

Modules touched:

- **`conversations` (server):** one new operation,
  `conversations.upload_draft_attachment`. It stores the bytes as a blob that
  the draft holds and adds the file to the draft's attachments. Sending
  already carries a draft's attachments by their blobs, so the send path
  doesn't change.
- **Client:** `lib/ui` `MessageComposer` and `DraftComposer` /
  `useDraftEditor`.

No new module and no cross-module dependency. Blob storage is the kernel's.

## What Changes

- **Attach (⇧⌘U)** opens the device's file picker. Files can also be dragged
  onto the composer or pasted into it, including a pasted screenshot.
- Each file shows while it uploads, then shows its name and size, and can be
  removed.
- The composer shows the total against the 25 MB a message may carry, and
  refuses a file that would go over before uploading it.
- **The server refuses:**
  - an empty file;
  - a file over 25 MB;
  - a draft whose attachments would pass 25 MB in total, or 20 files;
  - a draft that is no longer editable;
  - the file types Gmail and Outlook block, such as `.exe`, `.js` and
    `.bat`. Gmail would refuse to send them anyway.
- **File names** are cleaned: no path, no control characters, at most 255
  bytes. The media type sent is kept for the message but never trusted.
- **An upload is a change to the draft,** like a save. It names the draft's
  `version` and `content_token`, bumps the version, and answers with the
  draft. The client runs uploads in turn with its saves.
- Only the member (or API client) who owns the draft may upload. The
  assistant (Scout) can't.

## Non-goals

- Images inside the text (inline `cid:` parts).
- Picking files from Drive, Notion or Pages.
- Resumable or chunked uploads; the limit stays 25 MB.
- Showing upload progress as a percentage. The file shows "Uploading…"
  instead.
- Virus scanning.

## Impact

- `crates/modules/conversations`:
  - a new `features/upload_draft_attachment`;
  - `domain/drafts.rs`, which gains a `DraftAttachment.source`
    (`forwarded` | `uploaded`);
  - the route and body limit;
  - `update_draft`'s keep-list, which covers both kinds.
- OpenAPI and client types regenerated.
- Client: the composer's attach button, drop zone, paste handling, chips and
  total; `api/drafts.ts` `uploadAttachment`; `useDraftEditor` ordering.
- i18n in en, pt-PT, es-ES and fr.
