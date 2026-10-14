## MODIFIED Requirements

### Requirement: The need of a conversation
The module SHALL keep at most one open need per conversation, derived from the
newest inbound message sent within
`CAMPBOOKS_CONVERSATIONS__NEEDS_WINDOW_DAYS` (default 14), its decisions, the
policy and the conversation's state. A `sure` need SHALL come only from
answers the policy accepts: `pay` when `asks_action` is accepted with the
`payment_due` reason, else `reply` when `needs_reply` is accepted, else `do`
when `asks_action` is accepted. When answers of more than one source to the
same question were accepted, the rules' answer SHALL be the one used. A
`suspicious` answer of true SHALL raise a `review` need instead of any other.
When the `decide` role is configured, a model answer of true for
`needs_reply` or `asks_action` that the policy does not accept, with a
confidence of at least 0.8, where no `sure` need exists and no accepted
answer to the same question is false, SHALL raise a `review` need with trust
`check`. The need SHALL carry the due date resolved from an accepted
`has_deadline` answer, and `urgent` when an accepted `urgency` answer is
"today". Messages from the conversation's own inbox SHALL never raise one.

#### Scenario: A question from a person
- **WHEN** Marta writes "Could you confirm Thursday's walkthrough? The builders need to know by Wednesday." and the rules accept `needs_reply` and `has_deadline`
- **THEN** the conversation has a `sure` `reply` need due on the Wednesday after the message was sent

#### Scenario: A bill
- **WHEN** an invoice from a service says "Payment due by 30 October" and the rules accept `asks_action` with `payment_due`
- **THEN** the need is `pay`, due on 30 October

#### Scenario: Phishing asks for an action
- **WHEN** a message from a look-alike domain asks the member to verify their mailbox through a link, and the rules answer `suspicious` true and `asks_action` true
- **THEN** the need is `review` and no `do` need exists

#### Scenario: Old mail on the first sync
- **WHEN** a first sync brings a question sent 40 days ago
- **THEN** no need is raised

#### Scenario: Rules only
- **WHEN** no `decide` role is configured
- **THEN** no need has trust `check`

#### Scenario: The rules already settled it
- **WHEN** the rules accept `asks_action` false, `needs_reply` false and `has_deadline` false for a code host's notification, and the model answers `asks_action` true with confidence 0.875 and `has_deadline` true with 0.976
- **THEN** no need is raised

#### Scenario: Only the model says yes
- **WHEN** the rules give no accepted answer to `asks_action` and the model answers it true with confidence 0.875
- **THEN** the need is `review` with trust `check`

### Requirement: Clearing a need
A need SHALL be cleared, with its cause, when: a message is sent from the
inbox in the conversation after the message that raised it (`replied`; clears
`reply` and `do`), the conversation is archived (`archived`; clears `reply` and
`review`), every message is in the trash or spam or removed (`trashed`; clears
any), or a member resolves it. A new inbound message SHALL derive the need
again from that message (`superseded` for the old one). When the open need's
own message, read again, raises no need and no member confirmed it, the need
SHALL be cleared as `superseded`. Changes SHALL come from the provider or
from members alike.

#### Scenario: Replied from the phone
- **WHEN** Ana replies to Marta from Gmail on her phone and the sent message reaches the conversation
- **THEN** the `reply` need is cleared with cause `replied`

#### Scenario: Archiving a bill
- **WHEN** a member archives a conversation with a `pay` need
- **THEN** the need stays open until a member chooses Done or Not needed

#### Scenario: Done in a shared inbox
- **WHEN** Ana chooses Done on a need in an inbox she shares with Leo
- **THEN** the need is cleared for both of them

#### Scenario: A question raised against a settled answer
- **WHEN** a conversation has an open `check` need whose message's accepted answers say it asks nothing, and the conversation's need is refreshed
- **THEN** the need is cleared with cause `superseded`, announced once, and not shown under Done today

## ADDED Requirements

### Requirement: Spam and trash need nothing
A message in spam or the trash SHALL never raise a need: a conversation's
need SHALL come from its newest inbound message that is in neither, and a
need whose own message moves to spam or the trash SHALL be cleared as
`trashed`.

#### Scenario: The lease goes to the trash
- **WHEN** the message Pedro's `do` need comes from is moved to the trash, while his later message stays
- **THEN** the need is cleared with cause `trashed`

#### Scenario: A question in spam
- **WHEN** a question lands in spam inside a conversation whose need comes from an earlier message
- **THEN** the need still comes from the earlier message

### Requirement: Security alerts are a to-do
The email signals SHALL answer `asks_action` true with the reason
`signals.security_alert`, and `stream` `updates`, for a message whose subject
names a security problem (a security alert or advisory, a vulnerability, in
English, European Portuguese, Spanish and French; "action required" alone is
not one) only when all of these hold: the sender is a service (list mail or
an automated address); its domain is plain ASCII (no internationalised
label) and its display name names that domain; the receiving provider's own
authentication results, read as RFC 8601 says (comments and quoted strings
are not results), hold exactly one `dmarc` result, `pass`, whose own
`header.from` is the From domain, or a `dkim=pass` whose own signing domain
has the From domain's registrable domain; the message has no
phishing tell and carries no marketing words. It SHALL take precedence over
the list-mail "no action" answer. A sure `do` need from a security alert
SHALL require an authenticated sender on the listed code hosts and package
registries (`github.com`, `gitlab.com`, `bitbucket.org`, `npmjs.com`,
`pypi.org`, `rubygems.org`, `crates.io`, `snyk.io`), signing with the
platform's own name (`npm` for `npmjs.com`, `crates.io` for `crates.io`); for any other
authenticated sender the answer SHALL stay below every confidence the policy
acts from, so it raises at most a question. Anything that offers a message's
own links as the action on its need SHALL offer them only for a message
whose sender is authenticated this way and that is not suspicious.

#### Scenario: A dependency vulnerability
- **WHEN** "GitHub" `<notifications@github.com>` sends list mail titled "[example-org/demo-shop] High severity vulnerability in tiny-template" with Review, Dismiss and Upgrade links, the receiving provider reports `dkim=pass` and `dmarc=pass` for `github.com`, and the model also answers `asks_action` true with 0.875
- **THEN** the conversation is sorted to Updates and has a `sure` `do` need with basis `rules`

#### Scenario: The same alert, not authenticated
- **WHEN** the same alert arrives with no authentication results from the receiving provider, or with `dmarc=fail`
- **THEN** no `do` need is raised

#### Scenario: A crafted envelope sender
- **WHEN** a message from "Crates" `<alerts@crates.io>` fails DMARC, and its envelope sender `"a; dmarc=pass header.from=crates.io x"@evil.example` is echoed in the provider's results
- **THEN** `asks_action` is not answered by `signals.security_alert`

#### Scenario: A look-alike that signs its own mail
- **WHEN** "GitHub Security" `<alerts@githubsecurity.example>`, with `dkim=pass` and `dmarc=pass` for its own domain, sends "Security alert: new sign-in to your account"
- **THEN** no `sure` need is raised

#### Scenario: An internationalised look-alike
- **WHEN** the sender's domain spells "github" with a Cyrillic letter, or as `xn--` punycode
- **THEN** `asks_action` is not answered by `signals.security_alert`

#### Scenario: Action required alone
- **WHEN** an authenticated GitHub notification's subject says "Action required: review your settings" and names no security problem
- **THEN** `asks_action` is not answered by `signals.security_alert`

#### Scenario: A comment on a watched issue
- **WHEN** the same code host sends a notification of a comment on an issue the member watches
- **THEN** no need is raised

#### Scenario: Marketing that says action required
- **WHEN** a newsletter's subject is "Action required: claim your discount"
- **THEN** `asks_action` is false and the stream is Newsletters
