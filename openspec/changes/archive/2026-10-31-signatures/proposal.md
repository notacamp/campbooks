## Why

Mail sent from Campbooks carries no signature, so people type their name and
phone number by hand every time. This is phase 4 of the composer rework,
from the approved proposal
(https://claude.ai/artifact/BxLcKwjxF6gHbpfiWCeW8M).

Tracking: https://github.com/notacamp/campbooks-cloud/issues/393 (epic #389).

**Owner decision (2026-10-28):** a signature is text with the same marks as
a message (bold, italic, links), with no images and no HTML.

**Recorded refinement:** the proposal said "one per inbox". In a shared
inbox, though, each member signs with their own name. So a signature is
**the member's own, per inbox they send from**. Someone with one inbox has
exactly one, as approved. This also keeps one member from changing what
mail goes out under another member's name.

Modules touched:

- **`conversations` (server):**
  - a new table `conversations.signatures`;
  - two operations, `conversations.get_signature` and
    `conversations.set_signature`;
  - a `signature` field on drafts, filled when a draft is created and kept
    or left off by a save;
  - the send renders it.
- **Client:**
  - Settings → Mailboxes gets a Signature editor per mailbox you can send
    from;
  - the composer shows the signature under the text, with a way to leave
    it off.

No new module or cross-module dependency. The Settings page is in the
connectors client module and uses the conversations API through its public
client exports, as it already does for inboxes.

## What Changes

- **Settings → Mailboxes:** each mailbox you can send from gets
  "Your signature". It uses the same formatting editor as the composer
  (bold, italic, links), holds at most 2,000 characters, and saves with a
  toast.
- **A new draft** (reply, reply all, forward, new message or nudge) takes
  your signature for its inbox at creation.
- **The composer** shows the signature under your text, muted, with
  "Leave off" (and "Add signature" to bring it back) and "Edit in Settings".
- **On send,** the signature follows your text after a `-- ` line in the
  plain-text part, and in its own block in the HTML part, before the quoted
  history.
- What is sent is exactly the signature the draft showed. Editing your
  signature in Settings doesn't change drafts already started; leaving it
  off and adding it back takes the current one.

## Non-goals

- Images, logos or pasted HTML in signatures.
- Different signatures for new mail and replies.
- A workspace-wide signature that an admin enforces.
- Importing signatures from Gmail, Outlook or Zoho.

## Impact

- **Migration:** `conversations.signatures` (with RLS like its siblings) and
  `conversations.drafts.signature text NOT NULL DEFAULT ''`.
- **Domain:** the drafts domain, `create_draft`, `update_draft`
  (`signature: Option<bool>`), `binding::content_hash` (only when a
  signature is present, so tokens already issued stay valid) and
  `release_send` / `format::message`.
- **Two new operations,** plus OpenAPI, client types, tracking Coverage rows
  and i18n.
