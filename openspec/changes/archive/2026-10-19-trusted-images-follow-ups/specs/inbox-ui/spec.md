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
back again; once removed, Show images is back. A rule created, paused
or removed anywhere (Scout's memory, the assistant, another device, another
member of a shared inbox) SHALL show or hold back the images of the message
bodies the client holds without a reload, and a body a rule trusted SHALL
never load its remote images again from the copy the client kept before the
change. On the web, every change of a rule (made here, its live change, or
the change feed's reset) SHALL drop the kept bodies a rule trusted, so an
open one is read again behind its skeleton and a folded message, or a
conversation opened again, waits for a fresh read; it SHALL re-read in the
background the other kept bodies whose images are held back or offered. On
the desktop and mobile apps, until they follow the change feed live, a rule
changed in the app SHALL do the same; for a change made elsewhere, a body a
rule trusted SHALL be read again every time it is shown, every minute while
it is open and when the window regains focus, and the reader SHALL show the
kept body with its remote images held back (their sources taken out, and the
frame's policy blocking them) until the read made as it opened answers; a
body whose images are held back or offered SHALL be read again after a minute
or when the window regains focus. A member who only reads the inbox SHALL
not be offered it, nor SHALL anyone on mail that may be phishing. No visor:
no AI is involved.

#### Scenario: Trust the Ledger
- **WHEN** the member opens the Weekly Ledger's newsletter, presses Always show images from The Weekly Ledger, picks anyone at weekly-ledger.example and saves
- **THEN** a `show_images` rule for the domain is created, not applied to existing mail, and the reader shows the images with "Images shown: you trust The Weekly Ledger"

#### Scenario: Only reading
- **WHEN** the member's role in the inbox is reader
- **THEN** Show images is offered and Always show images is not

#### Scenario: What trusting an address means
- **WHEN** the member presses Always show images from ana@acme.example
- **THEN** the address choice says the images show for mail from that address when its domain's checks pass, and that those checks vouch for the domain, not for Ana alone

#### Scenario: Removed on another device
- **WHEN** a rule trusting The Weekly Ledger's images is removed on another device, or by another member of the shared inbox, while its newsletter is open here on the web
- **THEN** the open newsletter holds its images back again and offers Show images, without a reload

#### Scenario: Opened again after the rule is removed
- **WHEN** the member read the Weekly Ledger's newsletter on the web while a rule trusted its sender, folded it or left the conversation, and the rule is removed (here, on another device, or by another member)
- **THEN** opening it again shows its skeleton until it is read again, never its kept copy, and it then holds its images back and offers Show images

#### Scenario: Removed while the desktop app is open
- **WHEN** the same rule is removed on another device while the newsletter is open in the desktop app
- **THEN** within a minute, or as soon as the window regains focus, it holds its images back again and offers Show images

#### Scenario: Opened again in the desktop app
- **WHEN** the rule was removed on another device and the member opens the newsletter the desktop app kept
- **THEN** the kept newsletter shows at once with its images held back and no remote image source, and once it is read again its images stay held back and Show images is offered; had the rule still held, they would show once the read answered

