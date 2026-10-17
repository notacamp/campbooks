## ADDED Requirements

### Requirement: Trusted senders' images
A taught rule MAY have the action `show_images` ("Always show images from
<sender>"), only as its one action and with one condition, a sender address
or a sender domain (never a free mail domain alone); any other shape SHALL
be refused (`invalid_rule`). Creating or changing such a rule SHALL need
inbound mail from that sender (the address, or the domain and its
subdomains) in the rule's inbox, and SHALL start no run: it changes no mail.
`conversations.get_message_body` SHALL keep a message's remote images, as if
`show_images` had been asked, when an active rule of its inbox trusts its
sender and the receiving provider's authentication results authenticate that
sender: a DMARC pass for the From domain, or a DKIM pass aligned with it
that no DMARC result refuses, with no DMARC failure. No results, results
that cannot be read, `none`, a failed DMARC check and a spoofed From SHALL
never pass, nor SHALL mail whose conversation looks suspicious. The body
SHALL say `images_trusted` when a rule kept the images. Tracking pixels SHALL
be removed whether the images were trusted or not, and nothing else about
fetching or sanitising changes. Removing the rule SHALL hold the images back
again.

#### Scenario: Trusted and authenticated
- **WHEN** the member trusts news@brightloop.example and opens its newsletter, which passes DMARC, with a banner and a tracking pixel
- **THEN** the body keeps the banner, removes the pixel and says `images_trusted`

#### Scenario: Pretending to be the sender
- **WHEN** a message from news@brightloop.example fails DMARC, is signed by another domain, or carries no authentication results
- **THEN** its images stay held back

#### Scenario: A sender with no mail here
- **WHEN** the member asks to trust stranger@elsewhere.example, who never wrote to the inbox
- **THEN** the rule is refused with `invalid_rule`

#### Scenario: Removed
- **WHEN** the member removes the rule
- **THEN** the newsletter's images are held back until Show images
