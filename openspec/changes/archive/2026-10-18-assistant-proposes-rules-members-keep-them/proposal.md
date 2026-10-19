## Why

campbooks-cloud#242 item 13 (MEDIUM). Rules bind to the member who taught
them (#556) and a yes signs only what the member was shown (#561). Both
rest on one premise: a member's own click signs a rule. The operation
catalogue breaks it. `conversations.confirm_rule`, `update_rule` (adopt,
resume, reshape), `restore_rule` and `create_rule` are low-risk operations
the assistant may call (`x-campbooks.assistant`), and `get_rule` and
`list_rules` give it the member's `content_token`. So an assistant acting
for a member could read a proposal and say yes to it, or adopt, resume or
rewrite a kept rule, and the rule would be signed as the member's without
the member ever seeing it. Mail is untrusted input to the assistant, so a
prompt-injected message could get a planted or Scout-proposed rule turned
on and signed.

Two follow-ups from the #561 review ride along:

- item 11: a yes to a proposal kept the stored author
  (`coalesce(taught_by, $2)`), an author no screen shows and SQL may
  choose. A yes again already makes the confirming member the author.
- the ReplyDialog discards the draft it replaces without checking it is
  still a draft, and the server let a scheduled draft be deleted (it then
  never left), against the spec, which says a scheduled draft is not
  deleted.

## What changes

- The kernel's `Workspace<S>` extractor tells a handler who invoked the
  request in-process (`invoked_by`: the assistant or automation), from the
  request extension only `operations::invoke` attaches.
- `conversations.confirm_rule` is no longer the assistant's: the kernel
  refuses the call before the handler runs, confirmed or not. Its handler
  also refuses any invoked request (403 `rule_needs_your_yes`).
- `create_rule` stores what the assistant or automation saves as a
  proposal, whatever state, origin or preference, and refuses one that
  answers a suggestion (`notice_id`).
- `update_rule` refuses, for an invoked request, adopting, resuming and
  reshaping a kept rule; pausing and reshaping a proposal stay allowed.
- `restore_rule` refuses, for an invoked request, bringing a rule back
  active or paused.
- A yes to a proposal makes the confirming member its author.
  `paused_once_kept` therefore never names an author's reason any more.
- `conversations.delete_draft` refuses a scheduled draft (409
  `draft_not_editable`); `cancel_send` is the deliberate cancel. The
  ReplyDialog discards a replaced draft only when its state is `draft`.

## Capabilities

### New Capabilities

### Modified Capabilities

- `conversations`: only a member's own request says yes to a rule; a yes
  makes the confirmer its author; a scheduled draft is never discarded.
- `operation-catalogue`: a handler knows who invoked it.

## Impact

- Kernel: `tenancy::Workspace` gains `invoked_by` and `on_own_request()`.
- Conversations: `features/rules` (create, confirm, update, restore), the
  `paused_once_kept` computation, `update_draft::delete`, the server
  catalogue (`errors.rule_needs_your_yes`, four locales).
- OpenAPI: `confirm_rule`'s `x-campbooks.assistant` is false; the 403
  descriptions of the four rule operations, `delete_draft`'s summary and
  409, and `RuleView.paused_once_kept`'s description changed.
- Client: `ReplyDialog` guards its best-effort discard. No visible change.
- No migration.
