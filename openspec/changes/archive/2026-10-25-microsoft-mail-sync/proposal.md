# Proposal

## Why

A Microsoft mailbox can be connected since #315, but its stream only keeps
the grant alive: nothing is read, so an Outlook.com, Hotmail, Live, MSN or
Microsoft 365 mailbox shows no mail. This change is the second slice of the
Microsoft Graph epic: **reading** the mailbox, kept current by Graph's change
log per folder and, where the instance can be reached from the internet, by
Graph's change notifications. Write-back and sending (#317) follow as their
own change.

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/316
(epic https://github.com/notacamp/campbooks-cloud/issues/309).

Modules touched: `connectors` (the Graph messages adapter, its cursor, the
subscriptions table and the public callback), `server` (mounts the callback
next to the identity protocol endpoints). No new module, no new cross-module
dependency, no new crate. One new maintenance-scope file in
`architecture.toml` (the callback's subscription lookup).

## What Changes

- **The Microsoft messages adapter.** A Microsoft stream reads the mailbox's
  folders: Inbox, Sent Items, Junk Email, Deleted Items, Archive and the
  member's own folders (never Drafts, Outbox, Conversation History or Sync
  Issues), mapped to the contract the way the IMAP adapter maps folders
  (a `Campbooks/<Stream>` folder is a stream folder; the member's folders
  and Outlook categories are labels).
- **Delta per folder.** Each folder keeps its own cursor from
  `/me/mailFolders/{id}/messages/delta`: the first round reads the window
  (default 90 days), later rounds only what changed. A folder whose delta
  state Graph no longer has (410, `syncStateNotFound`, `resyncRequired`)
  starts a resync of the window, which keeps what is still there and removes
  what is gone. Message ids are Graph's immutable ids, so a move keeps the
  message.
- **Raw MIME from `$value`**, parsed by the shared parser and stored like
  every other adapter's, under the same size limit and per-run byte budget.
- **Read-only.** The sync never moves, deletes or marks anything at
  Microsoft: write-back is #317.
- **Throttling.** 429, or 503 with `Retry-After`, honours `Retry-After`
  (capped at an hour) and marks the connection `throttled`, as #315 does for
  refreshes; requests draw from a per-connection budget.
- **Change notifications, opt-in.** With
  `CAMPBOOKS_CONNECTORS__MICROSOFT__NOTIFICATIONS=true` and an `https`
  `CAMPBOOKS_PUBLIC_URL`, a run subscribes to the mailbox's messages and
  renews the subscription before it expires (about three days). Graph posts
  to the public, unauthenticated `/hooks/microsoft/mail`, which answers the
  validation handshake, checks each notification's `clientState` (a random
  secret per subscription, stored only as a keyed hash) in constant time,
  and asks for a run of that connection's stream only. Unknown subscriptions
  and wrong secrets get 404; the body is bounded. Without notifications (or
  without a public `https` URL) the stream is polled adaptively, silently.
- **Off by default.** All of it stays behind
  `CAMPBOOKS_CONNECTORS__MICROSOFT__ENABLED` (default false), and no
  Microsoft setting stops a server that does not offer Microsoft mail.

## Non-goals

- Write-back (read, star, archive, move, trash, categories) and sending:
  #317.
- Deleting anything at Microsoft, including a subscription at disconnect: a
  removed connection's subscription is forgotten and expires at Graph within
  three days, its notifications answered 404.
- Rich notifications (`includeResourceData`): the callback never carries
  mail.
- Focused / Other (`inferenceClassification`) as a stream.
- Microsoft calendar.
