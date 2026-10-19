# Tasks

## 0. Gate

- [ ] 0.1 `asks-lapse-or-go-stale` (public PR #607) merged; origin/next merged into this branch (never rebase), its MODIFIED requirements re-read against the merged baseline
- [x] 0.2 Owner approves the visual prototype (Nudge item, nudge dialog, Waiting on others; 375 + desktop, light and dark)

## 1. Decide (`campbooks-ai`)

- [ ] 1.1 `triage::NEXT_MOVE` and its four options; `triage::AWAITED` built from the awaitable people (`p1`…`p6`, `none`)
- [ ] 1.2 Facts: `awaitable(envelope)` (removes the inbox, the sender, machine addresses, duplicates; at most six), unit tests; the facts answer neither question
- [ ] 1.3 Both questions tolerant like the catalogue's (missing or off-options answer records nothing); confirm the provider's per-call question limit
- [ ] 1.4 Labelling guide rulings for `next_move` and `awaited`; eval items gain the two fields **unlabelled**, for the owner to label (never agent labels); `cargo xtask eval decide` reports them once labelled

## 2. Conversations: the wait

- [ ] 2.1 Migration: `needs.awaited_name`, `awaited_address`, `nudge_at`, `nudges`; reasons `waiting`/`nudge`, cause `heard_back` (checks widened, validated in a second migration); the hearing-back index
- [ ] 2.2 Need derivation: `waiting` after every other need, the 0.8 / accepted rules, the awaited person from the stored `awaited` options, trust `sure`/`check`; `next_move` takes away a model `reply`/`do` only when accepted
- [ ] 2.3 Nudge time: working days by UTC date at the message's time of day, `due_on` from Scout's read when given; `CAMPBOOKS_CONVERSATIONS__WAIT__WORKING_DAYS` (1 to 15, refused at start otherwise)
- [ ] 2.4 Need clock: a `waiting` need's clock row is its `nudge_at`; `wake_needs` turns it into `nudge` with a new revision
- [ ] 2.5 Hearing back on inbound store (same inbox, any conversation, after the need's message); re-arming on a send to the awaited address; archiving leaves the wait
- [ ] 2.6 `need_changed` v1 contract: the two reasons and the cause (additive); `cargo xtask events --check`
- [ ] 2.7 `ConversationSummary.need.waiting_on` and `nudges`; waits left out of "needs you" counts
- [ ] 2.8 Upgrade: queue every inbox once to ask the two questions for recent mail with no open need, in the daily AI limit

## 3. Conversations: the nudge

- [ ] 3.1 Draft kind `nudge` (reply all, awaited in To, the rest in Cc, never the inbox), 409 `not_waiting`
- [ ] 3.2 `conversations.write_nudge` on the `write_reply` machinery: scope, access, role and opt-in, limits, fixed instruction, mail as data
- [ ] 3.3 HTTP tests: addressing, read-only inbox, someone else's inbox (404), no model (409), rate limit, nothing stored

## 4. Attention

- [ ] 4.1 Reasons `waiting`/`nudge`; projection of `heard_back` and of a nudge going back to waiting
- [ ] 4.2 `attention.list`: waiting left out of the queue, Nudge ordered last, never dropped by age; `state=waiting`, soonest first
- [ ] 4.3 Replay and out-of-order tests for the new transitions

## 5. Home

- [ ] 5.1 Nudge group and row (both sentences, "Asked …", "Nudged once"), actions 1/E/S/X with Undo, read-only inbox
- [ ] 5.2 Nudge dialog (To/Cc as the server addresses it, Scout's text, Shorter, Change…, Send ⌘↵, Open in composer, Write my own, `reply_unavailable`), bottom sheet below 48rem
- [ ] 5.3 Waiting on others (aside on desktop, fold below 48rem, Nudge now, Not waiting)
- [ ] 5.4 Strings in en, pt-PT, es-ES and fr (diacritics checked); stories; component and page tests; axe

## 6. Close

- [ ] 6.1 OpenAPI regenerated
- [ ] 6.2 Client types regenerated
- [ ] 6.3 Changelog entry file in `changelog/unreleased/`
- [ ] 6.4 Gates green (CI)
- [ ] 6.5 Recording at 375px and desktop: a copied email → waiting → the clock → Nudge → sent → Ana writes → heard back
- [ ] 6.6 Cost per 1,000 emails measured before the owner switches it on in cloud
- [ ] 6.7 `openspec archive next-move-and-nudge --yes`
