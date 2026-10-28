## Decisions

### 1. Server

`SendDraft` gains `send_at: Option<DateTime<Utc>>` (`#[serde(default)]`).

- **When given,** it must satisfy
  `now + undo + 60 s <= send_at <= now + 366 days`, or the request answers
  400 `send_at_invalid`, before anything changes. The draft is then
  scheduled at `send_at`, with the same generation and binding. The
  `release_send` job is enqueued with `run_at(send_at)`.
- **When absent,** behaviour is unchanged.
- **Cancelling:** `cancel_send` already works on any scheduled draft until
  its release runs, and turns it back into an editable draft.
- **Binding:** the content binding (#242) is computed at send time, as now,
  and stays valid because a scheduled draft can't be edited.
- **Rate limit:** the `RateLimit::Send` class still applies when the send is
  requested.
- **Not assistant-callable:** the member's-own-request rule is unchanged, so
  the assistant can't schedule sends either.

### 2. The time

The client computes the instant in the device's time zone (`Intl`), which is
the zone the member sees, and sends it as UTC ISO.

- **"Next working day":** Monday to Thursday → the next day; Friday,
  Saturday or Sunday → Monday.
- **Pick a time:** defaults to tomorrow at 08:00, and accepts only future
  times inside the server's range. The form says why otherwise.

### 3. UI

- **Send ▾** is a split button: Send (⌘↵) plus a chevron that opens the
  menu. The menu uses the shared menu and popover components, in the top
  layer so the reader's pinned area can't clip it.
- **Each menu row** shows its label and the resolved time, e.g.
  "Tomorrow morning · Tue 29 Oct, 08:00".
- **Drafts' Scheduled group:**
  - shows drafts with `state = scheduled` whose `send_at` is more than
    2 minutes away, so undo-window sends don't flash in;
  - lists rows newest-leaving first, each with "Leaves Tue 29 Oct, 08:00"
    and a Cancel send button (toast "Send cancelled", with no Undo; schedule
    again from the composer);
  - opening a row offers "Cancel send and edit", which cancels the send and
    opens the draft in the composer.

## Data, events, scopes, AI

- **Data:** none new; `send_at` already exists on drafts.
- **Events:** the existing `DraftScheduled` tracked event.
- **Scope:** `conversations:send`, unchanged.
- **AI:** none.
- **Tracked events:** none added. No new operations, so no new Coverage
  rows.
