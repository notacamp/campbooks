## Why

"Worth a look" (campbooks-cloud#274) shipped with four gaps the owner approved
closing (campbooks-cloud#282):

- It raised Home items for mail whose sender fails authentication, so a spoofed
  message from a highlighted address could be put in front of the member as
  something they asked to see.
- Applying a rule to "the mail already here" raised an item for every unread
  match (41 in the demo), a backlog dump, when the intent was "see each one at
  least once".
- Its items counted in "N things need you" and in Home's badge, although it asks
  nothing of the member.
- A shared inbox's behaviour (one member's rule shows for everyone) was not
  written down or said in the confirmation.

## What Changes

- The engine raises `worth_a_look` only for mail whose sender is authenticated
  (the gate that shows remote images uses) and that is not suspicious, failing
  its checks or impersonated.
- Applying a Worth a look rule to existing mail raises one item: the newest
  unread matching conversation. New mail keeps raising one item per message.
  The preview and the confirmation say "the latest one already here".
- Worth a look items stay out of Home's hero count and the Home badge; the group
  keeps its own count. A Home with only Worth a look items says "Nothing needs
  you" and, below it, how many emails are worth a look.
- Shared inboxes: one member's rule raises the item for everyone in the inbox,
  and opening it clears it for all; the confirmation says so for Highlight.
- Cleanups: `worth_a_look` in `binding.rs`'s header, one query for a message's
  send time and read state in `needs::since`, and keyboard shortcuts on
  Scout noticed's four buttons (Shift + F, O, L, D).

## Impact

- Capabilities: conversations, home-ui, rules-ui.
- No migration. No change to the API shape.
