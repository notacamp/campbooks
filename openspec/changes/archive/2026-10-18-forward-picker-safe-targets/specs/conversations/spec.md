## MODIFIED Requirements

### Requirement: Who writes to the caller
`GET /api/v1/w/{workspace_id}/senders` (`conversations.list_senders`,
`conversations:read`, callable by the assistant) SHALL list senders of
inbound mail in the inboxes the caller can read: address, display name,
messages received and messages the caller's inboxes replied to; never a
subject or any text. Without `q` it SHALL return the 10 senders the
caller's inboxes replied to most (then received most); with `q` (2 to 100
characters) it SHALL return up to 10 senders whose address, display name
or domain contains each of its words, case- and accent-insensitively, in
the same order. `limit` SHALL be 1 to 10 (else 400). There SHALL be no
cursor. A sender seen only in inboxes the caller cannot read SHALL never
appear. It SHALL read only the newest inbound mail: the last 365 days, at
most 5,000 messages (through a partial index on received mail), so the
counts are within that window and its cost doesn't grow with an inbox's
history. Within one compile, the assistant SHALL ask it at most once per
`q`. With `reachable=true` it SHALL list only senders it is safe to write
to: never a no-reply address, nor a sender whose newest mail in that
window fails their sender checks or is refused by their DMARC policy, nor a
sender any of whose own mail in that window, in the caller's inboxes and
spam included, was reported (is in spam), is under review (open, or cleared
because a later message replaced it), or holds a stored `suspicious` answer
of `true` that no member corrected to safe. A flagged message SHALL exclude
only its own sender, so the other people in its thread stay. Facts only
(addresses, the provider's authentication results, stored answers, needs and
member corrections), never what the mail says. Without it, the answer SHALL
be as before.

#### Scenario: Suggestions for "always reach you"
- **WHEN** a member lists senders without `q`
- **THEN** the senders they replied to most come first, with their counts and no subjects

#### Scenario: Another member's inbox
- **WHEN** a sender wrote only to an inbox the caller is not in
- **THEN** that sender is not listed, for that caller or for a client token without the inbox

#### Scenario: Old mail only
- **WHEN** a sender's every message is more than a year old
- **THEN** that sender is not listed

#### Scenario: Safe to write to
- **WHEN** a member lists senders with `reachable=true`, with or without `q` ("support", "accounts")
- **THEN** neither it-support@at3lier-sol.example (under review) nor no-reply@accounts.example is listed, while both are found without it

#### Scenario: Reported once
- **WHEN** a member reported one of a sender's conversations and the sender's other conversation is unflagged
- **THEN** that sender is still not listed with `reachable=true`

#### Scenario: A review replaced
- **WHEN** a later message replaced a sender's review, or the review's need is gone, and no member said the flagged email is safe
- **THEN** that sender is still not listed with `reachable=true`; once a member says it is safe, they are

#### Scenario: Failing checks, newest
- **WHEN** a sender's newest mail is refused by their DMARC policy
- **THEN** they are not listed with `reachable=true` until newer mail of theirs passes
