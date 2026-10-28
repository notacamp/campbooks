# Proposal

## Why

The pre-merge review of Microsoft Graph write-back and sending (#317,
notacamp/campbooks#669) left follow-ups to close before Microsoft mail is
turned on in production. Each is a case where what Campbooks tells the member
differs from what Graph did: a send Graph accepted reported unknown (so it
may be sent twice by hand), a refused read taken for a deleted message, a
move with nowhere to go ending without a word, a trash that lost its restore
folder, changes left waiting when the flag is off, and a throttled send that
does not mark the connection.

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/367
(follows https://github.com/notacamp/campbooks-cloud/issues/317).

Modules touched: `connectors` only. No new module, dependency, crate,
migration, setting, event, operation or job.

## What Changes

- **An accepted send never ends `unknown_outcome`.** Once `sendMail`
  answered 202 the send is recorded sent even when its Sent Items copy cannot
  be found (filed late, another `Message-ID`, an id that cannot be looked
  up): under the message's own `Message-ID` as its provider id (what an IMAP
  send is known by, and the `send_completed` contract requires an id), and
  the next sync brings the copy. Attempts that did not reach a 202 keep the
  look-up-then-unknown rule.
- **A failed read is not "message gone".** Only a 404 at the read before a
  write means gone. A 400, a 403 or an unreadable answer ends the change
  `failed` (`refused`) and emits the message again.
- **A mixed change whose move has nowhere to go** emits the message again
  with Graph's state instead of ending `applied` in silence.
- **A trash move that went through unseen keeps its restore folder.** Where
  the message was is bound into the pending change before the move is
  called; a pending change bound with its place verifies like a pending one.
- **Pending changes when Microsoft mail is turned off** end `skipped`
  (`unsupported`) at the stream's next run, as the spec says.
- **A send that hits 429** marks the connection `throttled` and holds its
  stream until `Retry-After`, as a read does.
- **Tests** check that nothing was deleted after every write and send.

## Non-goals

- Bcc on a real Microsoft 365 mailbox: needs a live mailbox, checked by hand.
- The same gap for IMAP (a mixed patch-and-move change with nowhere to go
  ends `applied` without a re-emit): left alone here.
- Looking for a late Sent Items copy to record its id on the send.
