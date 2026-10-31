## Why

Every send leaves when its undo window ends. There's no way to write now and
have the mail leave tomorrow morning. This is phase 3 of the composer rework,
from the approved proposal
(https://claude.ai/artifact/BxLcKwjxF6gHbpfiWCeW8M).

Tracking: https://github.com/notacamp/campbooks-cloud/issues/392 (epic #389).

Owner decision (2026-10-28): the choices are Tomorrow 08:00, the next
working day 08:00 and Pick a time, in your time zone.

Modules touched:

- **`conversations` (server):** `conversations.send_draft` takes an optional
  `send_at`. A send is already a job that runs at `send_at` and can be
  cancelled until then, so nothing else changes on the server.
- **Client:** `lib/ui` `MessageComposer` (Send ▾), `DraftComposer`, and the
  Drafts list (a Scheduled group).

No new module or dependency.

## What Changes

- **Send gets a menu (Send ▾)** with three choices: Tomorrow at 08:00, the
  next working day at 08:00 (Monday when today is Friday, Saturday or
  Sunday), and Pick a time… (a date and a time). All three are in the
  device's time zone.
  - Choosing one schedules the draft, and the composer closes with a toast:
    "Scheduled for Tue 29 Oct, 08:00", with Undo.
  - ⌘↵ still sends now.
- **The server** accepts a `send_at` from one minute after the undo window
  up to 366 days ahead. Anything outside that answers 400 `send_at_invalid`.
  Without `send_at`, a send waits only for the undo window, as before.
- **Drafts** lists scheduled sends in their own group, "Scheduled", above
  the drafts. Each row shows when it leaves, plus "Cancel send", which turns
  it back into a draft. Opening one asks to cancel the send first, because a
  scheduled draft can't be edited.

## Non-goals

- Suggesting a time ("Scout picks when they usually reply").
- Recurring sends.
- Sending later from the Reply dialog. It keeps sending now; the composer's
  Send ▾ is the way to schedule.

## Impact

- `send_draft`: an optional `send_at` field, its checks, and the problem
  code. OpenAPI and client types regenerated.
- Client:
  - `MessageComposer` gets a split Send with a menu, plus a small date/time
    form (native `<input type=date>` / `<input type=time>`).
  - `DraftComposer` passes `send_at`.
  - `DraftsList` / `DraftsScreen` get the Scheduled group.
  - i18n.
