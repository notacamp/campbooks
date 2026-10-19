# Proposal

## Why

A saved draft can only be thrown away from inside the open composer. Once
the composer is closed (Esc) the reader shows a "Draft saved" chip that only
reopens it, a second draft on the same conversation (a reply and a forward,
say) is not shown at all, and the Home Reply dialog keeps a draft it has
saved with no way to delete it. Nothing says "deleted", and there is no way
back from a mis-click.

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/238

## What Changes

- Modules touched: the `conversations` client only. The server already has
  `conversations.delete_draft`; no endpoint, scope or event changes.
- The reader's closed composer lists every draft the member can still edit
  on the conversation, each as a chip that names it ("Draft saved · Reply to
  Maya Okafor", "Draft saved · Forward"), with its own **Delete draft**.
- The Home Reply dialog offers **Delete draft** in its draft stage once a
  draft exists (a resumed draft, or after Open in composer saved one), never
  before. It deletes the draft and closes the dialog.
- Every delete, the composer's Discard included, shows a "Draft deleted"
  toast with Undo. Undo writes the draft again with the same kind, message,
  recipients, subject, text and quoted history, as it was on screen
  (unsaved edits included). Forwarded attachments come back as a new
  forward carries them, so any the member had removed return.

## Non-goals

- No bulk delete, no trash for drafts, no server-side restore: Undo creates a
  new draft (a new id).
- No keyboard shortcut: the shortcuts catalogue has no natural fit.
- Scheduled and sending drafts are not deletable (they leave, or Undo takes
  them back).
