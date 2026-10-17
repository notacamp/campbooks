# Proposal

## Why

Part 1 (#546, #547) has the decide model rate each need's actions from a
catalogue and the server send at most three, ranked, as `need.actions`,
while `CAMPBOOKS_CONVERSATIONS__ACTIONS__ENABLED` is on. Home still offers
its fixed actions. This part makes Home offer the ranked ones, so the owner
can switch the catalogue on once the labelled eval has set the floor.

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/166

## What Changes

- Modules touched: `attention` (Home), `conversations` (a Forward dialog and
  the `list_senders` query it uses) and `lib/ui` (`ItemActions` takes a
  `picked` mark).
- A need with `actions` offers them as its own actions, in the server's
  order, on keys 1 to 3, in the existing row (one button style, the
  universal Done, Snooze and Not needed after the divider). The visor before
  them says Scout picked them; never a sparkle.
- A need without the `actions` key (the catalogue off, or the message not
  rated) offers exactly today's fixed actions. Only a missing key falls back.
- Reply, Report and delete, It's safe, Already paid and Still to pay run
  today's handlers (`use-need-actions`), with their Undo as now.
- Remind asks when in the Snooze dialog ("Remind me about <name>…"), a
  bill's reminder before its due date first; Forward asks to whom in a
  dialog that finds people by name (`conversations.list_senders`), never an
  address typed from memory, then opens the forward draft, addressed, in the
  reader; nothing is sent from Home. Open attachment opens the item on its
  email with the first file focused (Enter previews or saves it, as in the
  reader).
- A pair the server offers as one (the combo table, empty today) is one
  button that runs both, in order.
- A question (`check`) keeps Yes and No, and a bill already said to be still
  to pay never offers Still to pay again.
- Server (conversations, the catalogue's facts): an image the message's
  body shows (inline, with a Content-ID, a picture: a signature logo) is no
  attachment, so Open attachment is neither asked nor rated for mail that
  has only such images. The triage envelope is unchanged, and with the
  catalogue off nothing more is read. The combo table is checked when the
  crate builds: a pair never holds two actions that ask a follow-up in a
  dialog (Reply, Remind, Forward), and the client skips such a pair too.
- Forward re-addresses only a forward nobody wrote in yet; a warning offers
  only its answers whatever the server sends; Open attachment says so when
  there is no file to focus.
- Test-only: the Laya decide-pass test of #547 compares the needs
  themselves (question, reason, trust, basis, urgency, due date) with the
  catalogue on and off, not only their count.

## Non-goals

- No server change: the catalogue, its floor and the combo table are as
  part 1 left them, and the catalogue stays off by default.
- No new place to forward from other than Home; the reader's Forward is
  unchanged.
