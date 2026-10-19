# Tasks

## 1. Kernel

- [x] 1.1 `Workspace<S>::invoked_by` (and `on_own_request()`), from the `InvokedBy` extension only `invoke` attaches

## 2. Rules

- [x] 2.1 `confirm_rule`: no longer `.assistant()`; the handler refuses an invoked request (403 `rule_needs_your_yes`) after the access checks
- [x] 2.2 `create_rule`: an invoked create is always proposed; `notice_id` refused when invoked
- [x] 2.3 `update_rule`: an invoked adopt, resume or reshape of a kept rule refused; pause and reshaping a proposal allowed
- [x] 2.4 `restore_rule`: an invoked restore to active or paused refused
- [x] 2.5 `errors.rule_needs_your_yes` in the server catalogue, all four locales
- [x] 2.6 A yes to a proposal makes the confirming member its author; `paused_once_kept` models the confirmer as author
- [x] 2.7 OpenAPI: `confirm_rule` not the assistant's; the 403 descriptions; `paused_once_kept`'s description

## 3. Drafts

- [x] 3.1 `delete_draft` refuses a draft that is not editable (409 `draft_not_editable`)
- [x] 3.2 ReplyDialog discards the draft it replaces only while its state is `draft`

## 4. Tests

- [x] 4.1 The assistant's confirm, with the member's token, confirmed or not, is refused and nothing is signed; the member's own Keep still works and makes them the author (module and server)
- [x] 4.2 What the assistant saves is proposed, even for a member who lets Campbooks act; a `notice_id` is refused
- [x] 4.3 The assistant's adopt, resume, reshape of a kept rule and restore are refused with the binding generation, MAC and state unchanged; pause and remove work
- [x] 4.4 The assistant may reshape a proposal, which the member then keeps as it now is
- [x] 4.5 A yes makes the confirmer the author (existing tests updated: a proposal kept by another sender, a proposal whose author is away)
- [x] 4.6 A scheduled draft is not discarded, only cancelled; then it may be
- [x] 4.7 ReplyDialog never deletes a replaced draft that is scheduled
