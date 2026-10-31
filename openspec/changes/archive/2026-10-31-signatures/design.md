## Decisions

### 1. Data

The new table:

```
conversations.signatures (
  workspace_id uuid NOT NULL, inbox_id uuid NOT NULL REFERENCES conversations.inboxes ON DELETE CASCADE,
  user_id uuid NOT NULL, text text NOT NULL CHECK (char_length(text) <= 2000),
  updated_at timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY (inbox_id, user_id)
)
```

- RLS on `workspace_id`, like the other `conversations.*` tables. Copy the
  policy shape from the newest `conversations__*` migration.
- `conversations.drafts` gains `signature text NOT NULL DEFAULT ''`.
- One migration, named after the newest one on `next` at build time. Check
  open PRs so the version isn't taken.
- Member removal and workspace purge delete the member's rows. Hook into the
  existing `forget_member` / `purge_workspace` features.

### 2. Operations

Both are on `/api/v1/w/{workspace_id}/inboxes/{inbox_id}/signature`:

- **`conversations.get_signature` (GET):** returns `{ text }`, with `""`
  when unset. Scope `conversations:read`.
- **`conversations.set_signature` (PUT):** takes `{ text }` and answers with
  it. Scope `conversations:send`. A `text` over 2,000 characters answers 400
  `signature_too_long`.

Both:

- need the `sender` or `manager` role on the inbox. A `reader` gets 403
  `inbox_read_only`, and a non-member gets 404;
- need a member token: a client token with no user gets 403, because a
  signature is a person's;
- are not assistant-callable.

`set_signature` is `Risk::Low`. Its x-campbooks records are
`conversations.inbox`, with write access.

### 3. Drafts

- **`create_draft`** copies the author's signature for the draft's inbox
  into `drafts.signature`.
- **`DraftView`** gains `signature: String`.
- **`DraftChanges`** gains `signature: Option<bool>`:
  - `false` sets it to `""`;
  - `true` sets it to the author's current signature for the inbox;
  - absent leaves it alone.
  - It counts as a change for the version bump only when the value changes.
- **`content_hash`** appends `.str(&row.signature)` only when the signature
  isn't empty, so content tokens issued before this change stay valid.

### 4. Rendering on send

`format::message(body, signature, quoted)`:

- **Plain text:** `body`, then `"\n\n-- \n"` and `to_plain(signature)` when
  there is one, then `"\n\n"` and `quoted`.
- **HTML:** `body` with `Marks::Own`, then
  `<div class="signature">` + the signature with `Marks::Own` + `</div>`,
  then the quoted history with `Marks::Quoted`. All of it sits in the one
  outer `<div>`.

### 5. Client

- **Settings → Mailboxes:** each sendable inbox gets a "Your signature"
  section. It uses the composer's editor in a compact, toolbar-light variant
  (Bold, Italic, Link), with an explicit Save (⌘↵) and a toast "Signature
  saved".
- **Composer:** under the text, `— ` plus the signature rendered from its
  marks. It is read-only, muted, and comes with:
  - "Leave off", which saves `signature: false`;
  - "Add signature", shown when it's off and the member has one, which
    saves `signature: true`;
  - "Edit in Settings", a link to the mailbox's settings.
  - With no signature for that inbox, the composer shows only "Add a
    signature in Settings".

## Data, events, scopes, AI

- **Owner:** the `conversations` schema.
- **Events:** none published or consumed.
- **Scopes:** as above.
- **AI:** none. The reply-drafting prompt does not receive the signature;
  the draft body stays the member's own text.
- **Tracked events:** none added. Add Coverage rows for both new
  operations.
