## ADDED Requirements

### Requirement: Only a member's own request sends mail
Mail SHALL leave, and a send SHALL be taken back, only through a member's
own request: one that reached the API over HTTP with the person's token.
When the assistant, or an automation rule, runs an operation on a member's
behalf (`kernel::operations::invoke`), the server SHALL let it write drafts
and never send one or cancel a send:

- `conversations.send_draft` and `conversations.cancel_send` SHALL NOT be
  callable by the assistant or automation (`x-campbooks.assistant` and
  `.automation` false), so the kernel refuses the call before the handler
  runs, whether or not the call is marked confirmed.
- Both handlers SHALL refuse any invoked request that reaches them with
  403 `send_needs_you`, after the access checks (404 for a draft the caller
  cannot see, 403 for an inbox they only read) and before any 409, with a
  WARN line carrying the workspace and draft ids and the caller kind only.
  Nothing SHALL be scheduled, bound or released, and a scheduled send SHALL
  stay scheduled.
- `conversations.create_draft`, `update_draft`, `get_draft`, `list_drafts`
  and `delete_draft` SHALL stay callable by the assistant: what it writes
  is sent only by the member's own Send, with the `content_token` of the
  draft on their screen.

Whether a request was invoked SHALL be the kernel's to say
(`Workspace::invoked_by`); nothing a client sends SHALL set or clear it. A
member's own Send and Undo SHALL keep working unchanged.

#### Scenario: Scout sends with the member's token
- **WHEN** the assistant, acting for Sam, writes a reply, reads it back (its `content_token` is Sam's) and calls `conversations.send_draft` with its version and that token, as a call Sam confirmed or not
- **THEN** the call is refused before the handler runs; the draft is still a draft, with no send bound, no release queued and no send asked of the connector

#### Scenario: The flag flipped back by mistake
- **WHEN** `send_draft` or `cancel_send` were listed for the assistant again, and it calls one confirmed
- **THEN** the handler answers 403 `send_needs_you` and changes nothing

#### Scenario: Scout takes back Sam's send
- **WHEN** Sam's reply is scheduled and the assistant calls `conversations.cancel_send` for it
- **THEN** the call is refused, the send stays scheduled and leaves when the window ends

#### Scenario: Sam sends what Scout wrote
- **WHEN** Sam opens the draft Scout wrote and presses Send
- **THEN** it is scheduled with Sam's request bound to it, and his Undo can still take it back

### Requirement: Only a member's own request trusts a sender's images
A rule that shows a sender's images (`show_images`) SHALL be saved,
reshaped and brought back only through a member's own request. When the
assistant, or an automation rule, runs a rule operation on a member's
behalf, the server SHALL answer 403 `images_need_you`, after the access
checks and before anything is saved, to:

- `conversations.create_rule` whose actions show images, whatever state it
  asks for (so no proposal of the assistant's can make the member's own
  rule for that sender answer 409 `images_already_trusted`);
- `conversations.update_rule` that changes the conditions or actions of a
  rule that shows images, or changes a rule's actions to show them;
- `conversations.restore_rule` of a rule that shows images.

Pausing, declining and removing such a rule SHALL stay allowed. A member's
own "Always show images" SHALL keep working unchanged.

#### Scenario: Scout proposes trusting a sender
- **WHEN** the assistant, acting for Sam, saves "always show images from notifications@github.com", proposed or active
- **THEN** the answer is 403 `images_need_you` and no rule is saved

#### Scenario: Sam trusts the sender himself
- **WHEN** Sam then chooses "Always show images" for that sender
- **THEN** the rule is saved, active and Sam's, with no `images_already_trusted`

#### Scenario: Scout reshapes or restores one
- **WHEN** the assistant changes a proposal's actions to show images, or brings back an image rule Sam removed
- **THEN** each answers 403 `images_need_you` and nothing changes
