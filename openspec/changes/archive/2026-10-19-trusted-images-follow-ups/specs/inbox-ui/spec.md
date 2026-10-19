## MODIFIED Requirements

### Requirement: Always show images from a sender
Where a message's remote images are held back and the member may teach rules
in its inbox, the reader SHALL offer, beside Show images, "Always show images
from <sender>" (⇧A, the newest message from someone else). It SHALL open the
smart-rule confirmation: the rule as a sentence with the Smart rule tag,
that it runs on its own with no AI and lives in Scout's memory, a choice
between the sender's address and anyone at its domain (no domain for a free
mail provider), the address choice saying plainly that it holds when the
domain's checks pass and that those checks vouch for the domain, not for the
one person (they authenticate the domain, never the part before the @), how
many conversations it opens up, that it holds only for
mail passing the sender's checks and that tracking pixels stay removed, and
no offer to apply it to the mail already here. Saving SHALL show its result
and offer Undo. A message whose images a rule trusts SHALL say so ("Images
shown: you trust <sender>") instead of offering Show images. Removing the rule
in Scout's memory SHALL say, as it asks to confirm, that the images are held
back again; once removed, Show images is back. A rule created or removed
anywhere (Scout's memory, the assistant, another device, another member of a
shared inbox) SHALL show or hold back the images of the message bodies the
client holds without a reload: on the web at once, a live change of a rule
re-reading the bodies whose images depend on rules (trusted, held back or
offered); on the desktop and mobile apps, until they follow the change feed
live, within a minute or when the window regains focus. A member who only reads the inbox SHALL not be
offered it, nor SHALL anyone on mail that may be phishing. No visor: no AI
is involved.

#### Scenario: Trust the Ledger
- **WHEN** the member opens the Weekly Ledger's newsletter, presses Always show images from The Weekly Ledger, picks anyone at weekly-ledger.example and saves
- **THEN** a `show_images` rule for the domain is created, not applied to existing mail, and the reader shows the images with "Images shown: you trust The Weekly Ledger"

#### Scenario: Only reading
- **WHEN** the member's role in the inbox is reader
- **THEN** Show images is offered and Always show images is not

#### Scenario: What trusting an address means
- **WHEN** the member presses Always show images from ana@acme.example
- **THEN** the address choice says the images show for mail from that address when acme.example's checks pass, and that those checks vouch for acme.example, not for Ana alone

#### Scenario: Removed on another device
- **WHEN** a rule trusting The Weekly Ledger's images is removed on another device, or by another member of the shared inbox, while its newsletter is open here on the web
- **THEN** the open newsletter holds its images back again and offers Show images, without a reload

#### Scenario: Removed while the desktop app is open
- **WHEN** the same rule is removed while the newsletter is open in the desktop app
- **THEN** within a minute, or as soon as the window regains focus, it holds its images back again and offers Show images

