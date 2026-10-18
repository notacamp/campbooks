## Context

`kernel::operations::invoke` runs an operation in-process for a member, as
the assistant or as an automation rule. Before dispatching it checks that
the operation's `x-campbooks` opts the caller in, that a high-risk
operation is `confirmed`, and that a mutating one carries an idempotency
key. `confirmed` is a field of the `Invocation` the calling code builds
itself: it is no proof that the member clicked anything. #568 added
`Workspace<S>::invoked_by`, set only from the kernel-private `InvokedBy`
extension that `invoke` attaches (`None` on HTTP), and used it to keep
rules from being said yes to in-process. This change reuses that exact
mechanism for outbound mail.

## Inventory

Every operation that sends, schedules, releases, cancels or un-cancels
outbound mail, and every other `Risk::High` operation, as of `origin/next`
before this change. "Assistant" and "automation" are the operation's
`x-campbooks` flags; "after" is this change.

### Outbound mail

| Operation | Where | Risk | Assistant | Automation | After |
|---|---|---|---|---|---|
| `conversations.send_draft` (schedules a send after the undo window; sending again after a cancel is the "un-cancel") | `crates/modules/conversations/src/features/send_draft/mod.rs:65` | high | **yes** | no | assistant no; handler refuses any invoked request, 403 `send_needs_you` |
| `conversations.cancel_send` (takes a scheduled send back) | `crates/modules/conversations/src/features/send_draft/mod.rs:248` | low | **yes** | no | assistant no; handler refuses any invoked request, 403 `send_needs_you` |
| `conversations.create_draft` | `crates/modules/conversations/src/features/create_draft/mod.rs:67` | low | yes | no | unchanged: writing a draft sends nothing |
| `conversations.update_draft` (refuses a scheduled or sending draft, 409) | `crates/modules/conversations/src/features/update_draft/mod.rs:73` | low | yes | no | unchanged: a save re-issues the token, so a member's open screen then gets 409 `content_changed` (#561) and sees the change |
| `conversations.delete_draft` (refuses a scheduled or sending draft, 409, since #568) | `crates/modules/conversations/src/features/update_draft/mod.rs:179` | low | yes | no | unchanged: cannot take a send back |
| `conversations.write_reply` (the model writes reply text; saves nothing) | `crates/modules/conversations/src/features/reply/mod.rs:186` | low | no | no | unchanged |
| `conversations.release_send` (job: hands a scheduled send to the connector when the window ends, only at its own generation and only with a MAC that verifies, #552) | `crates/modules/conversations/src/features/release_send/mod.rs:51` | job | not an operation | not an operation | unchanged: only `send_draft` enqueues it |
| `conversations.check_send` (job: a released send that never reports back becomes `failed`) | `crates/modules/conversations/src/features/check_send/mod.rs:35` | job | not an operation | not an operation | unchanged |
| `conversations.record_send` (consumer of `connectors.send_completed`) | `crates/modules/conversations/src/features/record_send/mod.rs:21` | consumer | not an operation | not an operation | unchanged |

The connector sends only what the outbound-requests port says stands
(`features/outbound_requests`): a draft `sending` as that very send, with a
MAC `release_send` verified. So with `send_draft` closed to invokers there
is no other path from an invoked request to a provider send.

`workspaces.invite_member` also sends mail (an invitation); it is listed
below with the other high-risk operations.

### Other `Risk::High` operations

None of these opts the assistant or automation in, before or after this
change; `confirmed` never reached them.

| Operation | Where | Assistant | Automation |
|---|---|---|---|
| `connectors.disconnect` | `crates/modules/connectors/src/features/disconnect/mod.rs:51` | no | no |
| `identity.register_client` | `crates/modules/identity/src/features/register_client/mod.rs:58` | no | no |
| `identity.register_passkey` | `crates/modules/identity/src/features/register_passkey/mod.rs:40` | no | no |
| `identity.remove_client` | `crates/modules/identity/src/features/remove_client/mod.rs:29` | no | no |
| `identity.remove_passkey` | `crates/modules/identity/src/features/remove_passkey/mod.rs:26` | no | no |
| `identity.remove_way_in` | `crates/modules/identity/src/features/remove_way_in/mod.rs:35` | no | no |
| `identity.rotate_client_secret` | `crates/modules/identity/src/features/rotate_client_secret/mod.rs:34` | no | no |
| `identity.sign_out_everywhere` | `crates/modules/identity/src/features/sign_out_everywhere/mod.rs:23` | no | no |
| `workspaces.change_role` | `crates/modules/workspaces/src/features/change_role/mod.rs:39` | no | no |
| `workspaces.delete` | `crates/modules/workspaces/src/features/delete_workspace/mod.rs:48` | no | no |
| `workspaces.invite_member` | `crates/modules/workspaces/src/features/invite_member/mod.rs:87` | no | no |
| `workspaces.leave` | `crates/modules/workspaces/src/features/leave_workspace/mod.rs:25` | no | no |
| `workspaces.remove_member` | `crates/modules/workspaces/src/features/remove_member/mod.rs:32` | no | no |
| `bff.admin_post` / `_put` / `_patch` / `_delete` (the web BFF's admin proxy) | `crates/server/src/bff/handlers.rs:778` | no | no |

The admin API (`openapi/admin.json`) is not in the catalogue `invoke`
reads, so no invoker reaches it.

## Decisions

### Sending and cancelling are the member's own request only

Both layers, as #568 did for `confirm_rule`:

1. `x-campbooks.assistant` false on `send_draft` and `cancel_send`: the
   kernel refuses the invocation (`NotAllowed`) before any handler runs,
   whatever `confirmed` says. This is the layer that acts today.
2. In each handler, `members_own_send` refuses any request whose
   `invoked_by` is set with 403 `send_needs_you`, logging a WARN with the
   workspace and draft ids and the caller kind only. It runs after
   `drafts::load` (the 404 for a draft the caller can't see, the 403 for a
   read-only inbox) and before the 409s (`draft_not_editable`,
   `draft_changed`, `content_changed`, `too_late`), so a refusal leaks
   nothing a 404 would hide and changes and signs nothing. It holds even if
   the flag is ever flipped back, and it refuses automation the same way.

The assistant keeps `create_draft`, `update_draft`, `get_draft`,
`list_drafts` and `delete_draft`: writing is its job. A member sends what
it wrote with their own HTTP Send, with the `content_token` of the draft
on their screen (#561), and the release is bound to that request (#552).
A save by the assistant re-issues the token, so a member whose screen
predates it gets 409 `content_changed` and sees the change before sending.

### The `confirmed` flag

After this change no catalogued high-risk operation opts the assistant or
automation in, so `confirmed` gates nothing an invoker can reach. Rather
than rely on it, `cargo xtask arch` now fails on any high-risk operation
whose `x-campbooks` opts the assistant or automation in: adding one back
needs a change to that rule, which is reviewed like an ADR change. The
kernel's own check (`ConfirmationRequired`) stays as a second wall and the
field stays, since the kernel's unit tests use it and removing it is a
wider API change with no security gain now; its docs say plainly that the
caller sets it and that it proves no click. A future flow where a member
confirms an assistant's high-risk call needs a proof bound to that click
(like the #561 content token), and a new decision record.

Changed: `send_draft` (assistant false; handler guard), the xtask rule,
and the docs of `Risk` and `Invocation::confirmed`. No other high-risk
operation needed a change.

### Item 18: the assistant files no show-images rule at all

Two options: let the member's own create pass over an assistant's
proposal (count only kept rules in `not_trusted_yet`), or refuse the
assistant's proposal outright. Refusing is safer:

- showing a sender's remote images tells that sender the mail was opened
  (pixels are removed, but any image fetch is a signal), and the request
  would come from mail, which is untrusted input to the assistant;
- a proposal is shown in Scout's memory with words the caller wrote
  (item 15), so an injected "always show images from …" would read as a
  sensible suggestion one click from being kept;
- with no assistant proposals there is nothing to block the member, and
  the one-rule-per-sender invariant (`images_already_trusted`) stays
  simple.

So `create_rule`, `update_rule` when it reshapes a rule whose old or new
actions show images, and `restore_rule` of such a rule answer 403
`images_need_you` to an invoked request, after the access checks and
before anything changes. Pausing, declining and removing an image rule
stay allowed: they make it act less. The member's own "Always show
images" (an HTTP request) is unchanged.

### Item 17

The reply dialog discards the draft it replaces when its state is `draft`
or `failed` (both editable, and the server deletes both); a `scheduled` or
`sending` one is never discarded there, only its Undo takes it back.

## Risks / trade-offs

- An assistant that later gains tool calling cannot send at all, not even
  after asking: that is intended until a member's confirmation is bound to
  their click.
- Automation could not send before either (`automation` was false on
  both); the handler guard now also says so in code.
- The assistant may still discard an editable draft (`delete_draft`) or
  rewrite one (`update_draft`); neither sends, and the second is visible to
  the member through the token. Left as is.
- No migration, and no data changes: nothing in production invoked these.
