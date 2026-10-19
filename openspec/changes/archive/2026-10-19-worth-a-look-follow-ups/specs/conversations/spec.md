## MODIFIED Requirements

### Requirement: Worth a look
A taught rule with `worth_a_look` SHALL answer a question `worth_a_look` (`true`,
source `taught`) on each new inbound message that matches it, and, when applied
to the mail already there, on the newest unread matching conversation only
(never on every unread match). A
conversation whose newest inbound message carries that answer and raises no
other need (a reply, a task, a bill or a warning comes first: one need per
conversation) SHALL have a need with reason and question `worth_a_look`,
`trust` `sure`, basis `taught`, never urgent and never a question. It SHALL
not pin, archive, mark read or move the mail: the conversation stays where
it was and can be archived like any other. The need SHALL clear itself as
`opened` once that message is read, here or at the provider, so marking a
conversation read or unread (`conversations.mark_read`, `mark_unread`) SHALL
work its need out again; a member's Seen is Done, with no correction filed.
`surface` never wins over `worth_a_look` (nothing raised).

#### Scenario: Highlighting a digest
- **WHEN** Sam's active rule `worth_a_look` is applied to 12 unread digests from one sender
- **THEN** only the newest of the 12 has an open `worth_a_look` need, and none is archived or read

#### Scenario: New digests
- **WHEN** two more digests from that sender arrive afterwards
- **THEN** each has its own open `worth_a_look` need

#### Scenario: Opened once
- **WHEN** Sam opens the highlighted one
- **THEN** its need is cleared with cause `opened`

#### Scenario: A reply comes first
- **WHEN** a highlighted sender's message also needs a reply
- **THEN** the conversation's need is Reply, not Worth a look

## ADDED Requirements

### Requirement: Worth a look only for authenticated senders
The engine SHALL raise `worth_a_look` only for a message whose sender the
receiving provider's authentication results authenticate (a DMARC pass for the
From domain, or an aligned DKIM pass; no results at all is not a pass), and
never for mail that looks suspicious, fails its sender checks or is
impersonated, whoever the rule names. This SHALL hold on arrival, when a rule is
applied to existing mail and in the preview, so the count a member is shown
matches what happens. The need itself SHALL be checked the same way when it is raised, so a
highlight answer stored earlier on mail that is not authenticated, or looks
suspicious, raises nothing.

#### Scenario: A spoofed message
- **WHEN** a rule highlights builds@ci.example.com and a message arrives from that address whose results authenticate another domain
- **THEN** no `worth_a_look` need is raised for it

#### Scenario: No results
- **WHEN** a message from a highlighted address carries no authentication results
- **THEN** no `worth_a_look` need is raised for it

#### Scenario: Authenticated
- **WHEN** a message from the highlighted address passes its checks and is not suspicious
- **THEN** its conversation has an open `worth_a_look` need

#### Scenario: The newest one is not authenticated
- **WHEN** a rule is applied to existing mail whose newest unread match is spoofed
- **THEN** the newest authenticated unread match is the one raised, and the spoofed one raises nothing

### Requirement: Worth a look in a shared inbox
A Worth a look need belongs to the conversation, so a rule taught by one member
of a shared inbox SHALL raise the item for every member who can read that inbox,
and opening the message (by anyone, here or at the provider) SHALL clear it for
all of them.

#### Scenario: Shared
- **WHEN** Sam teaches a Worth a look rule in the inbox she shares with Ana, and mail arrives
- **THEN** the item shows on Ana's Home too, and when Ana opens the message it clears on Sam's Home
