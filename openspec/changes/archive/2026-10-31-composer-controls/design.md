## Decisions

### 1. From

`DraftChanges.inbox_id: Option<InboxId>`. A change of inbox:

- requires the `sender` or `manager` role on the target inbox, in the same
  workspace (a `reader` gets 403 `inbox_read_only`, a non-member 404) and a
  target that isn't read-only;
- clears `thread_key`, and keeps `in_reply_to`, `references`,
  `conversation_id` and `message_id`;
- recomputes the From address and name;
- is a member's request or the assistant's: `update_draft` is already
  assistant-callable, and choosing From doesn't send anything.

The content hash already includes `inbox_address`, so the next send must
name the token issued after the move. The client lists inboxes with
`useSendingInboxes`, and shows the picker only when there is more than one.

### 2. Mode switch (client only)

The composer's title becomes a menu button: "Reply ▾ to Maya Okafor". The
choices are Reply (R), Reply all (⇧R) and Forward (F), the reader's own
keys, shown as hints but only active inside the menu.

Choosing one runs four steps:

1. `createDraft({ kind, message_id, quoted })`, with the quote built as
   `ReplyComposer` builds it today;
2. `saveDraft` of the new draft with `body` (and `subject` if the member
   changed it);
3. `deleteDraft` of the old one, best effort;
4. open the new draft in the composer.

If step 1 or 2 fails, the old draft stays open, with an error. The text is
carried over as marks, unchanged.

### 3. Scout menu (client only)

The menu shows only for `kind ∈ {reply, reply_all, nudge}`, when the
workspace's reply suggestions aren't `reply_unavailable`.

- **Rewrite request:** `streamReply` (or `writeNudge` for a nudge) with
  `message_id` = the draft's `message_id` and `draft` = the editor's text as
  plain text (`toPlain`). It also sends:
  - Shorter → `shorter: true`;
  - Warmer → `change: t("scout.warmer")` ("Make it warmer; keep what it
    says");
  - Fix spelling → `change: t("scout.spelling")` ("Fix spelling and grammar
    only; change nothing else");
  - Change… → a one-line ask in the composer (the `AskLine` the Reply
    dialog uses).
  - `intent` is required by the operation, so the composer sends
    `t("scout.keep")` ("Keep what my draft says"). The prompts aren't
    changed, so no A/B run is needed.
- **While Scout writes:**
  - the text is read-only, and a ThinkingTrace shows;
  - the result replaces the text as plain paragraphs. It is inserted as
    ProseMirror text nodes, never through `insertContent(string)`, which
    parses HTML, and never through `fromMarks`;
  - the editor's own undo, plus a toast with Undo, restores the previous
    document.
- **Errors** use the Reply dialog's messages (limit, daily limit, timeout).

### 4. Subject on replies

`withSubject` is `true` for every kind. The Re:/Fwd: prefix the server adds
stays editable.

## Data, events, scopes, AI

- **Data:** no new data; `update_draft` gets the new field.
- **Events:** none.
- **Scope:** `conversations:send`.
- **AI:** the existing `complete` role through the reply stream. When it
  isn't configured the menu is hidden. The member undoes a rewrite with
  Undo or ⌘Z.
- **Tracked events:** none added. No new operations.
