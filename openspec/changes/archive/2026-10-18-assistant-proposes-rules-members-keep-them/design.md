# Design

## 1. Threat model

The assistant calls operations by id through `kernel::operations::invoke`
as the member it acts for: the same router, scope and membership checks,
the member's access. It reads mail, and mail is untrusted, so whatever the
assistant may call, a message may ask it to call. After #556 and #561 a
rule acts only when signed, and the signers that keep row content sign
only what the member's client was shown (the `content_token`). But the
token is issued to the member, the assistant reads as the member, so the
assistant gets the member's token for every rule it can read. A yes from
the assistant would sign a rule as the member's with nothing ever on the
member's screen.

## 2. Inventory

Every operation the assistant could reach that creates, confirms, adopts,
resumes, changes or brings back a rule, or ends in the rules MAC signer
(`domain::rules::binding::sign`) or in a rule taking effect. Lines are on
`origin/next` at 6eb33023, in
`crates/modules/conversations/src/features/rules/mod.rs` unless named.

| Operation | Where | Assistant / risk before | What it signs or turns on | After |
|---|---|---|---|---|
| `conversations.create_rule` | :640 | yes, automation yes, Low | an active rule (signed, applied to existing mail); a `notice_id` keeps a suggestion acting; origin `correction`/`suggestion` acts when the member lets Campbooks act without asking | invoked: always `proposed`, unsigned, applied to nothing; `notice_id` refused 403 |
| `conversations.confirm_rule` (proposal) | :919, signs at :1001 | yes, Low | the proposal, signed and applied, with the stored author (:993) | not the assistant's (catalogue); handler refuses any invoked request 403; author = confirmer |
| `conversations.confirm_rule` (yes again) | `confirm_again` :1032 | same | a kept rule that does not verify, signed as is | same refusal |
| `conversations.update_rule` adopt | :1174, signs at :1285 | yes, Low | the rule with the adopter's rights | invoked: 403 |
| `conversations.update_rule` `paused: false` | same | yes, Low | a paused rule acting again | invoked: 403 |
| `conversations.update_rule` conditions/actions of a kept rule | same | yes, Low | the new content, signed | invoked: 403 |
| `conversations.update_rule` `paused: true` | same | yes, Low | the rule, paused (signed again as paused) | allowed: it acts less |
| `conversations.update_rule` reshaping a proposal | same | yes, Low | nothing (a proposal is never signed) | allowed: still waits for a yes, to what it now shows |
| `conversations.restore_rule` back to active or paused | :1449, signs at :1560 | yes, Low | the rule, signed and acting again (and, asked, applied) | invoked: 403 |
| `conversations.restore_rule` back to proposed | same | yes, Low | nothing | allowed |
| `conversations.decline_rule` | :1081 | yes, Low | nothing (a proposal goes) | allowed |
| `conversations.remove_rule` | :1319, signs at :1395 | yes, Low | the removed rule (kept bound for Undo); a revert run (MAC'd, #559) | allowed: it acts less, and putting mail back is what the member's own tools (spam, trash, move) already let the assistant do |
| `retire` (a rule replaced) | :855, signs at :885 | via create/confirm/update/restore only | the replaced rule, removed | unreachable for the assistant: its creates are proposals, its updates never activate |
| `rule_runs::start` (apply, revert) | `domain/rule_runs.rs` | via the operations above | a run of a rule just signed or verified (#559) | follows them |
| `conversations.preview_rule`, `get_rule`, `list_rules`, `suggest_rules` | :358, :1652, :1610, `suggest_rules/mod.rs:162` | yes (reads) | nothing; `get_rule`/`list_rules` carry the member's `content_token` | unchanged: the token is useless to the assistant now |
| `conversations.answer_rule_notice`, `clear_rule_notice_answer`, `list_rule_notices` | `rule_notices/mod.rs:575, :639, :445` | no | an answer (no rule) | unchanged |
| `assistant.compile_instruction` | `assistant/src/features/compile/mod.rs:95` | no (it calls reads as the assistant) | nothing: proposals for the member to preview and save | unchanged |
| `assistant.update_profile` ("Do it and tell me") | `assistant/src/features/profile/mod.rs:83` | no | the preference that lets evidence rules act | unchanged; and an invoked create ignores it |
| `conversations.resign_rules` (one-off job) | `resign_rules/mod.rs:149` | no (a job) | rules from before #556 whose hash matches the migration's | unchanged |

Nothing in production invokes a rule mutation as the assistant today: the
only in-process assistant calls are `compile_instruction`'s reads. The
exposure is latent, for the first tool-calling Scout, which is exactly when
it must already be closed.

## 3. Approaches

1. **Mark the four operations `Risk::High`.** The kernel then refuses an
   invocation not marked `confirmed`. This is the mechanism the assistant
   has for high-risk actions, but `confirmed` is a flag the in-process
   caller sets on its own `Invocation`: nothing ties it to a member's click
   on what the member saw. A confused or injected assistant loop that sets
   it signs anyway. It also blocks pausing and removing, which are safe.
2. **Drop `.assistant()` from every rule mutation.** Structural, but too
   wide: pausing, removing, declining and reshaping a proposal are useful
   and harmless, and `create_rule` must stay so Scout can propose.
3. **The handler refuses the signing paths when the request was invoked**,
   from a fact only the kernel can set, and the catalogue drops
   `confirm_rule` for the assistant entirely. The proposal state is the
   member-confirmation mechanism: Scout saves a proposal, the member sees
   it under "Waiting for you" (the rules list, with its token) and keeps
   it with their own click.

We take 3. It is deterministic, server side, and needs no second
confirmation system: the existing proposal and Keep are the confirmation.
`confirm_rule` loses `.assistant()` so the kernel refuses it before any
handler code runs (belt), and the handler refuses an invoked request too
(braces), so re-adding the flag by mistake, or opting automation in later,
still signs nothing.

## 4. Telling the assistant from the member

`operations::invoke` attaches two request extensions: the
`AuthenticatedPrincipal` and `InvokedBy(caller)`. Both types are
`pub(crate)` in the kernel. `Workspace<S>` now copies `InvokedBy` into a
public `invoked_by: Option<Caller>` field; HTTP requests never carry the
extension, so they read `None`.

- **A client cannot claim to be the member.** Request extensions are not
  headers: nothing on the wire sets or clears them. The BFF and the
  idempotency layer forward the request as it came and add only the
  principal. A forged `InvokedBy` cannot be made outside the kernel, and
  `Workspace<S>` has a private field, so no module builds one either.
- **The assistant cannot pass as the member** for the same reason: it runs
  through `invoke`, which always attaches its caller kind.
- A person's own app (a token they allowed) arrives over HTTP and counts as
  the person, as everywhere else in the API (ADR 0017). Client tokens are
  already refused for every rule mutation (`rule_needs_member`).

## 5. Item 11: the confirmer becomes the author

`confirm` now sets `taught_by` to the confirming member, as `confirm_again`
does, instead of `coalesce(taught_by, $2)`. Consistency with the MAC and
token:

- The token (`content_token`) covers the row as stored, author included,
  and is verified under the lock before the write, as before: it proves the
  member saw this proposal.
- `sign` then binds the row as written: the confirmer as author, the new
  state and `confirmed_at`. The MAC's inputs are all set by this request or
  verified by the token. Nothing the member did not see or set is signed.
- `paused_once_kept` used to warn that a proposal by an author who is away
  would stay paused once kept. The confirmer is now its author and may
  write in the inbox (confirming takes that), so only an inbox kept
  read-only or a domain too broad can still be said, and both refuse the
  yes anyway. The field and the UI line stay; the server no longer sends
  `author_unavailable` there.

This reverses #197's "confirming keeps its author". The reason is the
#242 principle: a rule acts with the rights of the member whose click
signed it, not of an author stored by SQL and never shown.

## 6. The draft nit

The UI cancels a scheduled send only through `cancel_send` (the Undo in the
send toast, the composer's and the reply dialog's). No screen deletes a
scheduled draft on purpose: the composer's Discard is for the draft it
edits, and a scheduled draft reached there (an old link) could not be
saved either. The reply dialog's best-effort discard of a replaced draft
was the one accidental path. So the
server's `delete_draft` now refuses a draft that is not editable (scheduled
or sending, 409 `draft_not_editable`, as the spec already said), and a
deliberate cancel is `cancel_send` followed, if wanted, by a delete. The
ReplyDialog keeps the replaced draft (not only its id) and discards it only
while its state is `draft`.

## 7. Judgement calls

- **Automation is treated like the assistant.** `create_rule` is
  automation-callable; a rule that creates active rules from mail would be
  the same injection. An invoked create of either kind is a proposal.
- **Pausing and removing stay the assistant's.** They make a rule act less.
  A removal with `restore` runs a revert, which puts mail back where the
  member had it before the rule: the assistant can already move, unspam
  and untrash mail directly, so it gains nothing. Bringing a rule back
  acting is refused.
- **Reshaping a proposal stays the assistant's**: it signs nothing, and the
  member's yes covers what the proposal holds then (its token is over the
  new content).
- **`confirm_rule` is out of the catalogue for the assistant** rather than
  high-risk: a confirmed invocation still is not the member's click.
- **Refusal is 403 `rule_needs_your_yes`**, after the access checks (404
  and 403 first, as the access matrix expects) and before conflicts.
- **`send_draft`** has the same shape (the assistant can read the member's
  draft token), but it is `Risk::High` and out of scope here (items 2 and
  13 are about rules); noted for a follow-up.

## 8. Residual risk

- An app the person allowed (a third-party token for their workspace)
  acts as the person, so it can still say yes to a rule it read. That is
  the API's model: such an app is the person's own client.
- A future tool loop could ask the member "Keep this rule?" in its own UI
  and call `confirm_rule` over HTTP with the member's session; that would be
  a member click, by design.
- `confirmed_at` outside the token (item 12) is unchanged.

## 9. Deploy and rollback

No migration. Nothing in production invokes these operations as the
assistant, so no flow changes behaviour except two: a yes now makes the
confirmer the author (a proposal by someone else is the confirmer's once
kept; managers and its new author may change it), and a delete of a
scheduled draft answers 409 (no screen does it). Rollback is redeploying
the previous image.
