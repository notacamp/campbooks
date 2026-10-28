## MODIFIED Requirements

### Requirement: Who the caller exchanges mail with
`GET /api/v1/w/{workspace_id}/correspondents` (`conversations.list_correspondents`,
`conversations:read`) SHALL list the people the inboxes the caller can read
exchange mail with: the senders of their inbound mail, the To and Cc
recipients of their inbound mail, and the To and Cc recipients of their sent
mail. Each person SHALL carry their address (lower-cased), the newest display
name seen for it in any of these, `wrote` (sent messages that had them in To
or Cc, each message once) and `received` (inbound messages from them); never
a subject or any text. `q`
SHALL be required, 2 to 100 characters; every one of its words (at most
five, split on spaces and commas) SHALL be in the person's address or a
display name seen for them, case- and accent-insensitively, and `%`, `_` and
`\` in it SHALL match themselves. `limit` SHALL be 1 to 10 (default 10). A
`q` or `limit` out of range SHALL get 400 `invalid_correspondents_query`.
There SHALL be no cursor. The answer SHALL be ordered by `wrote`, then
`received`, then the inbound messages that had them in To or Cc (each
message once), all most first, then by address. It SHALL read only the newest
mail of those inboxes: the last 365 days, at most 5,000 inbound messages
(not in spam or the trash) and at most 5,000 sent messages, newest first,
each through a partial index, so its cost doesn't grow with an inbox's
history. The addresses of the inboxes it reads SHALL never be listed, and
mail in inboxes the caller cannot read SHALL never count. A person the
caller's inboxes wrote to SHALL always be listed when `q` finds them. A
person who wrote in and was never written to SHALL be listed only when
`list_senders` with `reachable=true` would list them: not a no-reply address,
their newest mail in the window not failing their sender checks nor refused
by their DMARC policy, and none of their own mail in the window reported,
under review or holding a `suspicious` answer of `true` no member corrected.
A person only copied on inbound mail (never written to, never wrote in) SHALL
be listed only when they are not a no-reply address, none of their own mail
in the window is flagged as above, and the newest inbound message that had
them in To or Cc came from a sender who may be written to: that message not
failing its sender's checks nor refused by their DMARC policy, and none of
that sender's mail in the window flagged as above. Those checks SHALL run
person by person in ranking order, only until the answer is full, so their
cost doesn't grow either. Facts only (addresses, display names, the
provider's authentication results, stored answers, needs and corrections),
never what the mail says.

#### Scenario: By name or by domain
- **WHEN** Sam asks with `q=okáfor`, and then with `q=northwind`
- **THEN** the first lists maya@northwind-studio.example as "Maya Okafor", and the second lists Maya first and then leo@northwind-studio.example, copied on Maya's email

#### Scenario: Copied on received mail
- **WHEN** Maya's email to Sam had Leo Park in Cc, and Sam never wrote to Leo and Leo never wrote to Sam
- **THEN** `q=leo` lists leo@northwind-studio.example as "Leo Park" with `wrote` 0 and `received` 0

#### Scenario: Copied after those who wrote
- **WHEN** one Rivera wrote to Sam once, another was copied on three emails Sam received, and a third on one
- **THEN** `q=rivera` lists the one who wrote first, then the one copied three times, then the one copied once

#### Scenario: Copied by someone not to write to
- **WHEN** a sender under review copied helpdesk@at3lier-sol.example and Sam never wrote to that address
- **THEN** no `q` lists it, until Sam writes to it

#### Scenario: Only written to
- **WHEN** Sam wrote to dana@harbour-books.example and Dana never wrote back
- **THEN** `q=dana` lists her with `wrote` 1 and `received` 0, with the name Sam's email gave her

#### Scenario: Written to first
- **WHEN** Sam wrote twice to one Rivera and another Rivera wrote to Sam three times
- **THEN** `q=rivera` lists the one Sam wrote to first

#### Scenario: Not someone to write to
- **WHEN** a no-reply address and a sender under review both match `q`, and Sam never wrote to them
- **THEN** neither is listed

#### Scenario: Written to, whatever they sent
- **WHEN** Sam wrote to a sender whose own mail Sam later reported
- **THEN** that sender is still listed for a `q` that finds them

#### Scenario: Another member's inbox
- **WHEN** someone exchanged mail only with an inbox the caller is not in
- **THEN** they are not listed for that caller, until the caller is added to that inbox

#### Scenario: Never yourself
- **WHEN** Sam asks with `q=sam`
- **THEN** sam@example.com, the inbox's own address, is not listed, though it is in To of the mail Sam receives

#### Scenario: Out of range
- **WHEN** a caller asks with `q=a`, with `q` of only spaces, or with `limit=11`
- **THEN** the response is 400 `invalid_correspondents_query`

#### Scenario: Wildcards are letters
- **WHEN** a caller asks with `q=%_`
- **THEN** only people whose address or name holds "%_" are listed, so nobody in the fixtures
