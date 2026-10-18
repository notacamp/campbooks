# Proposal

## Why

Items 2 and 10 of campbooks-cloud#242, from the reviews of public #552 and
#559. An attacker with SQL as the runtime role inside workspace W, but not
the instance secret, can no longer send mail or make a rule act on their own
(#552, #556, #559). They can still change what a member's own click signs:

- **(2)** add a Bcc, or change a recipient or the body, on a member's open
  draft (or a draft Scout wrote) without bumping `version`. The member
  clicks Send and `conversations.send_draft` signs the row as it is, so the
  attacker's content leaves in the member's name.
- **(10)** swap a Scout-proposed rule's conditions or actions (or its
  author) between the member reading it and saying yes.
  `conversations.confirm_rule` signs what is stored.

After this change, Send and Keep sign only content whose hash matches a
token the server gave that member's client when it showed them the content.

Modules touched: `conversations` (the token, the checks, the views) and the
web client (`clients/app`: the composer, the reply dialog, the rules list,
the message composer component). No migration, no new endpoint, event,
scope, dependency or secret; one new field on two views
(`DraftView.content_token`, `RuleView.content_token`), one new request
field on three operations, one new problem code (`content_changed`).

## What Changes

- **A content token comes with what a member is shown.** Every response
  that shows a draft (read, list, create, save, send, undo send) or a rule
  (read, list, create, confirm, change, remove, restore, decline) carries
  `content_token`: `0x01` and an HMAC-SHA256 under the binding key of
  campbooks-cloud#227, with its own label
  (`campbooks/conversations.content-token/v1`), over the workspace, the
  member, the object kind and id, a draft's inbox id, its version (a draft) or binding
  generation (a rule), and the same content hash the signing step binds: the
  #552 draft content hash, or the #556 hash of a rule's conditions and
  actions with its inbox, stored state and author. It is never stored.
- **Signing, and every save, requires it back.** `send_draft`,
  `update_draft` (every save keeps fields it cannot send: the inbox, the
  reply headers, the attachments' files), `confirm_rule` (a proposal's yes
  and a yes again) and `update_rule` with `adopt` (unless it sets the whole
  rule) take `content_token`. A save that holds issues the next token over
  what it left, so the chain runs from the client's first read to the send. Under the row lock, after every access
  check, the server recomputes the hash and verifies the token in constant
  time; a missing, another member's or a stale token answers 409
  `content_changed` with no state change and a WARN carrying ids only.
- **The client sends back the token of what is on screen** and, on 409
  `content_changed`, reads the draft or rule again, shows it as it is now
  with a short notice, and never sends on the member's behalf. A refused
  autosave keeps what was typed, reads the draft again, says so once, and
  saves nothing on its own. The reply dialog never reuses a draft it did
  not create: it creates one with exactly the recipients it shows, no
  copies, its own quoted text and no attachments. The message composer
  shows Cc and Bcc whenever there are any.

## Non-goals

- **Before the client's first read.** What SQL changes in a draft before
  the member's client first reads it is in what it is given. Recipients,
  Cc, Bcc, text, quoted text, attachment names and From are shown; the
  reply headers, thread, the file behind an attachment's name and a
  reply's subject in the reply composer are not, so those can be signed.
  Any change after that read, to any field, is refused at save and Send.
  A re-read after a 409 is a read too (see `design.md` §7).
- **Replay of a state a member was shown** (the same limit as #552 §8): SQL
  that restores a row exactly as a member was shown it, version included,
  makes that member's token hold again. It can only make them send what
  they saw.
- **The rest of campbooks-cloud#242** (items 3 to 7: replay of requests,
  stream lookups, Zoho's `restore_to`, re-reads, key rotation), and the
  follow-ups filed separately (a proposal's author kept on confirm,
  `confirmed_at` outside the token, the assistant confirming with the
  member's token).
- **Old tabs.** A tab still running the client from before the deploy
  shows its generic error on save, Send or Keep until it is reloaded.

## Impact

- SQL in a workspace can no longer change what a member's Send or Keep
  signs, or what a save keeps, once the member's client has read the draft
  or rule.
- At the deploy, a composer or rules list opened before has no token: its
  first save, Send or Keep answers 409, the content is read again, typed
  text is kept, one notice shows, and the next save or click works. Scheduled sends, signed under #552, are not
  affected.
- Without an instance secret nothing can be signed, as before (500).
