# Design

## 1. Threat model

An attacker has SQL as the runtime role inside workspace W (row-level
security holds them to W) but not the instance secret. After
campbooks-cloud#227 (#552), the rule binding (#556) and the run binding
(#559), SQL alone sends nothing and no rule of theirs acts. Two signers
still sign what the row holds when a member clicks:

- **(2)** `conversations.send_draft` signs the draft as stored. Its only
  guard was `version`, which SQL also controls: add a Bcc, change a
  recipient or the body without bumping it, and the member's Send signs the
  attacker's content.
- **(10)** `conversations.confirm_rule` signs a proposal (or a rule that
  needs a yes again) as stored when the member says yes. SQL can swap its
  conditions, actions or author between the view and the yes.

After this change both sign only content whose hash matches a token the
server gave that member's client when it showed them that content.

## 2. Inventory

### Responses that show a member what they will send or confirm

| Response | Shows | Token |
|---|---|---|
| `conversations.get_draft` | the draft the composer opens | yes |
| `conversations.list_drafts` | the drafts a composer reopens (a reply slot picks one) | yes |
| `conversations.create_draft` (compose, reply, reply all, forward, the reply dialog's Send, Open in composer) | the new draft | yes |
| `conversations.update_draft` (autosave, the reply dialog's save, the forward dialog's recipient) | the draft as saved | yes |
| `conversations.send_draft` | the scheduled draft | yes (harmless: not editable) |
| `conversations.cancel_send` (Undo) | the draft again | yes; the client reopens the composer from it |
| `conversations.write_reply`, `suggest_replies` (Scout drafts) | text only, nothing stored | none: the text becomes a draft through `create_draft`/`update_draft` |
| `conversations.get_rule`, `list_rules` (Scout's memory, rules waiting for a yes) | each rule | yes |
| `create_rule`, `confirm_rule`, `update_rule`, `remove_rule`, `restore_rule`, `decline_rule` | the rule afterwards | yes |
| `conversations.preview_rule` (Teach Scout, the smart-rule confirmation, trusted images #549, rule offers) | what a rule the client holds would do | none: the client sends the whole rule to `create_rule` |
| `suggest_rules`, `list_rule_notices` (Scout noticed, assistant proposal cards) | rule shapes held by the client | none: saved through `create_rule` with the whole rule |
| `assistant.compile_instruction` | rule shapes held by the client | none: same |

### Requests that sign

| Request | Signs | Content from | Token |
|---|---|---|---|
| `send_draft` (and its schedule: the undo window is the schedule) | the send request (#552) | the row | **required** |
| `update_draft` (every save: autosave, the reply dialog's save of its own draft, the forward dialog's recipient) | nothing, but its answer issues the next token | the request's fields, and the row for every field it does not send (always some: the inbox, the reply headers, the attachments' files) | **required**: a save may only keep what was shown (§4) |
| `confirm_rule`, a proposal | the rule (#556) | the row | **required** |
| `confirm_rule`, a rule that needs a yes again (`confirm_again`) | the rule, now taught by the confirming member | the row | **required** |
| `update_rule` with `adopt` | the rule, now with the adopting manager's rights | the row (verified under #556's MAC) | **required**, unless the request sets both conditions and actions |
| `create_rule` (incl. a notice's smart rule, Teach Scout, the assistant's saves) | the rule | the request: the whole rule | not needed |
| `update_rule` setting conditions and actions | the rule | the request | not needed |
| `update_rule` pausing, resuming, or setting one of the two | the rule | the row, whose #556 MAC verified under the lock | not needed (§6) |
| `remove_rule`, `restore_rule`, `retire` (a rule replaced) | the rule in its new state | the row, verified under the lock | not needed (§6) |
| `rule_runs::start` (#559) | the run | the rule just signed or verified | not needed |
| `release_send`, `conversation::request` | the stored send, change requests | already bound (#552) | not needed |

## 3. The token

`domain::binding::ContentToken` in the conversations module, under the
kernel `Binding` (the #227 key, HKDF label `campbooks/send-binding/v1`) with
a new context label, `campbooks/conversations.content-token/v1`:

```
content_token = base64url( 0x01 ‖ HMAC-SHA256(key,
    label, workspace, member (opt), kind,
    draft: id, inbox_id, version, content_hash(row)              -- #552's hash
    rule:  id, binding_generation, content_hash(conditions, actions), -- #556's hash
           inbox, stored state, taught_by (opt) ) )
```

- **One hash.** The draft's is `domain::binding::content_hash`, the very
  function `send_draft` binds into the send request; the rule's is
  `domain::rules::binding::content_hash`, the one its MAC binds. The token
  adds the rule's inbox, stored state and author as fields, not a second
  encoding of its content. `send_draft` computes the hash once and passes
  it to the request it signs.
- **Bound to the member** (`Access.user`): another member's token does not
  hold. Client tokens have no member; they already share the drafts client
  tokens write, and cannot confirm or adopt rules.
- **Bound to the version or generation**: a token of before a save, or of
  before a rule was signed again, does not hold.
- **Kind** is in the encoding, so a draft's token is never a rule's, and
  the id, so another draft's or rule's never holds.
- **The draft's inbox id** is a field of the token beside the content hash
  (which covers the inbox's address and name), as the send request binds
  it. The #552 content hash itself is unchanged, so sends scheduled before
  the deploy still release.
- Never stored, logged or put in an event. Without an instance secret no
  token is issued (`content_token` absent) and nothing signs (500).

## 4. The checks

`send_draft`: after `drafts::load` (access, row lock), the editable check
and the `version` check (409 `draft_changed`), `shown_to_author` recomputes
`content_hash(row)` and verifies the token, in constant time
(`Binding::verify`). It refuses 409 `content_changed` with a WARN carrying
the workspace and draft ids and `why = missing|mismatched`, before any
write: no state change, no request MAC, no release job, nothing published.

`update_draft`: the same check (`drafts::shown_to_author`), at the same
place, against the row *before* the write. Every save keeps fields the
request cannot send (the inbox, the reply headers and thread, the
attachments' files, sizes and types: attachments are named by id), so
every save takes the token; there is no full replace that needs none. A
missing, foreign or stale token answers 409 `content_changed`: nothing
written, an ids-only WARN. Only a save that holds writes, and its answer
issues the next token, over what the save left. So the token chain runs
from the response a client first read the draft from, through every save,
to the send, and a save can no longer re-issue a token over fields SQL
changed after that read. Without an instance secret no token exists and
nothing can be signed: saves are not checked, and `send_draft` refuses.

`confirm_rule`: after the access checks, the runs' and the rule's locks and
the reload under the lock, a proposal and a rule that needs a yes again are
verified the same way (`shown_to_caller`) before `validate`, `replace` or
any write. A rule that is neither still answers 409 `rule_not_proposed`
first. `update_rule` with `adopt` checks after the `rule_needs_confirmation`
check.

## 5. Client

- **The composer** (`use-draft-editor`) keeps the token of the content on
  screen in a ref, from the draft it opened with, then from each answer it
  applies: a save's (the save sends every field the composer edits) or a
  re-read after a 409. With the same answer it takes what it shows but
  does not edit (From, the attachments' names). Every save and Send sends
  the token with the version.
- **An autosave refused** (409 `content_changed`, or `draft_changed` from
  another tab) saves nothing. The composer keeps what the member typed,
  reads the draft again for its version, token and attachments, shows
  "This changed since you opened it. Check it and send again." once, and
  saves nothing on its own: the next edit, or Send, saves the typed text
  with the fresh token. No loop: one refusal, one read.
- **A send refused** reads the draft again and shows it as it is now
  (a send saves first, so nothing typed is lost), with the same notice.
  It never sends again by itself.
- **The message composer** shows Cc and Bcc whenever there are any, also
  ones that arrive after it opened, so a re-read draft's Bcc is on screen.
- **The reply dialog** (Scout drafts) never reuses a draft it did not
  create. It used to save into an empty reply draft from the drafts list,
  or the draft an Undo handed back, keeping that row's subject, quoted
  text, attachments and headers, none of which it shows. Now it creates a
  fresh reply draft with exactly the recipients it shows, no Cc or Bcc and
  its own quoted text, and discards the draft it replaces (the undone one).
  It saves again only the draft it created, with that draft's token,
  sending the recipients, its quoted text and no attachments. After a 409
  it says so, sends nothing, and the next Send creates a fresh draft.
- **The forward dialog** saves the recipient into an untouched forward
  draft with that draft's token; refused, it starts a fresh forward. Both
  open in the composer, which shows the whole draft before Send.
- **The rules list** sends the rule's token with Keep. On 409 it says
  "This rule changed since you opened it. Check it and keep it again." and
  the list it refetches shows the rule as it is now.
- No other screen saves or sends a draft, or confirms a rule.

## 6. Judgement calls

- **Which requests need the token.** Those whose signed content comes from
  the row without a MAC vouching for it: a send, a proposal's yes, a yes
  again. Adopting is added: the rule is verified by #556, but adopting makes
  it act with another member's rights, so that member vouches only for what
  they were shown. Pausing, resuming, partial changes, removing and bringing
  back keep content whose #556 MAC verified under the lock; requiring a
  token there would change nothing an attacker can do, and would break the
  generic Undo (`restore_rule`) that carries no screen.
- **Missing is mismatched.** The brief and the safe default: a request
  without a token is refused, so an API client or the assistant must read
  first. The assistant acts as the person, so the token it reads is theirs.
- **Error order.** Access (404/403) first, then the existing conflicts
  (`draft_not_editable`, `draft_changed`, `rule_not_proposed`,
  `rule_needs_confirmation`), then `content_changed`, then validation
  (400). The access matrix keeps answering every caller as declared.

## 7. Residual risk

What is enforced: Send signs a draft only if its row, as stored, hashes to
the content of the last response that client applied (the draft as read,
listed or created, a save's answer, or a re-read after a 409), for that
member, at that version and inbox. A save writes only under the same
condition. A yes or an adoption signs a rule only if it is the rule the
member's client last read. What that leaves:

- **Before the first read.** SQL that changes a draft before the member's
  client first reads it is in what that client is given, so it can be
  signed. Fields on screen (recipients, Cc and Bcc, subject on a new
  message or forward, text, quoted text, attachment names) are then shown;
  the inbox's address is shown as From. Fields no screen shows (the reply
  headers and thread, an attachment's file behind its name, a reply's
  subject in the reply composer) are not. The token stops any change after
  that read.
- **A re-read is a read.** After a 409 the client reads the draft again
  and takes its token: what changed by then is shown where the composer
  shows it, and hidden fields are taken as they are.
- **Replay** of a row exactly as the member was shown it makes their token
  hold again (#552 §8): it can only send what they saw.
- **A list landing at the click.** The rules list updates the sentence and
  its token together; the member says yes to what the row showed.
- **Not covered here** (filed separately): a proposal's author kept on
  confirm, `confirmed_at` outside the rule token, the assistant confirming
  rules with the member's token.

## 8. Deploy and rollback

No migration. At the deploy an open composer, reply dialog or rules list
holds no token. The composer's first autosave (or Send) answers 409: it
keeps what was typed, reads the draft again, shows the notice once, and
the next save or Send works with the fresh token; it never retries on its
own, so there is no loop of 409s. The reply dialog's first Send after an
Undo from before the deploy creates a fresh draft and needs no old token.
Keep in the rules list answers 409 once and works after the refetch. A tab
still running the old client shows its generic error on save, Send or
Keep until reloaded; its text stays in the editor, and the last save
before the deploy is kept. Scheduled sends were signed under #552 (whose
content hash is unchanged) and release as before. Rollback is redeploying
the previous image: the field is ignored by the old server.
