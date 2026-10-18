## ADDED Requirements

### Requirement: What the reader may offer about a sender's images
`conversations.get_message_body` SHALL say, for a message whose remote images
were held back, whether trusting its sender can take effect
(`images_offer`: no rule of the inbox, paused or waiting included, covers the
sender yet, the email is not under review, its provider shares sender checks
and they pass), the sender's registrable domain that may be offered instead
of the address (`images_domain`: owned, never a free mail or internet
provider's, nor a platform anyone can send from), and why the images stay held
back although the sender is, or could be, trusted (`images_held`:
`under_review`, `checks_fail`, `unverified` when the provider checked nothing,
`cannot_authenticate`, `not_checked` while Scout has not checked the email
under an acting rule, or `rule_paused` when the rule covering the sender is
paused or waiting for a yes). A message with no remote images SHALL offer and
explain nothing. The body SHALL read the inbox's rules only when one of them
shows images.
Creating or changing an image rule for a sender a rule already covers (the
same address, or a domain that covers it) SHALL be refused with 409
`images_already_trusted`, with the engine deciding whether an existing rule
covers it. An image rule for anyone at a platform anyone can send from
(Substack, Mailchimp, Google Groups and the like) SHALL be refused as invalid.
The free mail list SHALL be one list, compared by
registrable domain (`mail.yahoo.fr` is Yahoo's), covering the major global and
regional providers and internet providers' mail; the web client's copy SHALL
be checked against it.

#### Scenario: Offered with its domain
- **WHEN** an authenticated newsletter from news@brightloop.example has its images held back and no rule trusts the sender
- **THEN** `images_offer` is true and `images_domain` is `brightloop.example`

#### Scenario: Could not take effect
- **WHEN** a copy fails DMARC, or carries no authentication results
- **THEN** nothing is offered and `images_held` is `checks_fail`, or `cannot_authenticate`

#### Scenario: Already trusted
- **WHEN** a rule trusts news@brightloop.example and the member asks for it again
- **THEN** it is refused with 409 `images_already_trusted`, and while Scout has not checked the email it says `not_checked`

#### Scenario: Under review
- **WHEN** a trusted sender's email has a stored `suspicious` answer of `true`
- **THEN** its images stay held back and `images_held` is `under_review`

#### Scenario: A platform anyone sends from
- **WHEN** a member asks to show images from anyone at `substack.com`
- **THEN** the rule is refused, and the reader offers only the address
