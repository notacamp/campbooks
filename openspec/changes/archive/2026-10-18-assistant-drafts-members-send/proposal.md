## Why

campbooks-cloud#242 items 14, 17 and 18, follow-ups from the review of
notacamp/campbooks#568.

- **Item 14 (HIGH, latent).** `conversations.send_draft` is the one
  high-risk operation the assistant may call (`x-campbooks.assistant`).
  The kernel lets it through when the code making the call sets
  `confirmed` on its own `Invocation`, and nothing ties that flag to a
  member's click. The assistant may also read a draft (`get_draft`, which
  carries the member's `content_token`) and create and save drafts. Mail
  is untrusted input to the assistant, so a future tool-calling Scout
  could be talked into sending mail in the member's name. It may also call
  `conversations.cancel_send`, so a message could get a send the member
  made taken back. Nothing in production invokes either today (the only
  in-process caller, compiling an instruction, reads inboxes, labels and
  senders); this closes it before anything can.
- **Item 17 (info).** The reply dialog discards the draft it replaces only
  while it is a `draft`, so a `failed` one, which is just as editable, is
  left behind.
- **Item 18 (LOW).** A show-images rule the assistant proposes holds back
  the member's own "Always show images" for that sender
  (`images_already_trusted`) until the member declines the proposal.

## What changes

- `send_draft` and `cancel_send` are no longer the assistant's
  (`x-campbooks.assistant` false), so the kernel refuses the call before
  the handler runs, confirmed or not. Their handlers also refuse any
  invoked request (`Workspace::invoked_by`, the mechanism of #568) with
  403 `send_needs_you`, after the access checks and before the 409s. The
  assistant may still create, save, read and discard drafts; the member
  sends one with their own Send and its content token (#561).
- `cargo xtask arch` fails on any high-risk operation that opts the
  assistant or automation in. After this change none does, so the
  caller-set `confirmed` flag gates nothing an invoker can reach.
- An invoked `create_rule`, an invoked `update_rule` that reshapes a rule
  into or out of showing images, and an invoked `restore_rule` of such a
  rule answer 403 `images_need_you`. Only the member's own request trusts a
  sender's images, and the assistant can file no proposal that stands in
  its way.
- The reply dialog discards the draft it replaces while it is a `draft` or
  a `failed` send, never one that is scheduled or sending.

## Impact

- Code: `conversations::features::send_draft` (both handlers),
  `conversations::features::rules` (create, update, restore), the
  conversations server catalogue (two messages, four locales),
  `xtask::arch` (the high-risk rule), kernel doc comments, `ReplyDialog`.
- API: `send_draft` and `cancel_send` leave the assistant's operations;
  new 403 codes `send_needs_you` and `images_need_you`. No client calls
  these as the assistant, and members' requests are unchanged.
- No migration.
