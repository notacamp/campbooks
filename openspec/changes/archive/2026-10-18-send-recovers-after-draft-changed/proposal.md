# Proposal

## Why

Item 19 of campbooks-cloud#242, from the review of public #571. A member
has a draft open with nothing unsaved, and the draft is saved somewhere
else: another tab, a second device, later the assistant. Its version moves
on, so Send answers 409 `draft_changed`. The composer's Send only read the
draft again on `content_changed`, so it kept the old version and every
later Send failed the same way, with a notice telling the member to close
the composer and open it again. The reply dialog had the same gap: a
`draft_changed` from its own save or from Send was reported as a generic
send failure, and the dialog kept saving the same stale draft. The forward
dialog gave up ("Couldn't start the forward") when the untouched forward it
re-addresses had been saved elsewhere since.

## What Changes

- **The composer's Send treats `draft_changed` like `content_changed`.**
  It reads the draft again and shows it as it is now, with its version and
  token, and the one-time notice "This changed since you opened it. Check
  it and send again." Nothing is sent on the member's behalf; the next Send
  names the fresh version and token. Text typed while the refused Send was
  in flight is kept: only the version and token are taken, and the next
  save writes the typed text over them.
- **The reply dialog treats `draft_changed` like `content_changed`,** from
  its save of the draft it created or from Send: it says the reply changed,
  sends nothing, and the next Send writes what is shown into a fresh reply
  draft, discarding the old one while it is editable (the #561 and #571
  behaviour).
- **The forward dialog starts a fresh forward** when saving the untouched
  forward answers `draft_changed`, as it already did for `content_changed`.
- The composer's `draft_changed` notice ("Close it and open it again") is
  no longer reachable and is removed from the four locales.

Modules touched: the web client only (`clients/app`, conversations
module). No server change: the server already answers `draft_changed`
before `content_changed` and the client already had everything it needs to
recover (`get_draft`). No migration, endpoint, event, scope or dependency.
