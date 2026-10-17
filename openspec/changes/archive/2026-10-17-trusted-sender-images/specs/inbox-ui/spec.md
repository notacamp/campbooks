## ADDED Requirements

### Requirement: Always show images from a sender
Where a message's remote images are held back and the member may teach rules
in its inbox, the reader SHALL offer, beside Show images, "Always show images
from <sender>" (⇧A, the newest message from someone else). It SHALL open the
smart-rule confirmation: the rule as a sentence with the Smart rule tag,
that it runs on its own with no AI and lives in Scout's memory, a choice
between the sender's address and anyone at its domain (no domain for a free
mail provider), how many conversations it opens up, that it holds only for
mail passing the sender's checks and that tracking pixels stay removed, and
no offer to apply it to the mail already here. Saving SHALL show its result
and offer Undo. A message whose images a rule trusts SHALL say so ("Images
shown: you trust <sender>") instead of offering Show images. Removing the rule
in Scout's memory SHALL say, as it asks to confirm, that the images are held
back again; once removed, Show images is back. A member who only reads the inbox SHALL not be
offered it. No visor: no AI is involved.

#### Scenario: Trust the Ledger
- **WHEN** the member opens the Weekly Ledger's newsletter, presses Always show images from The Weekly Ledger, picks anyone at weekly-ledger.example and saves
- **THEN** a `show_images` rule for the domain is created, not applied to existing mail, and the reader shows the images with "Images shown: you trust The Weekly Ledger"

#### Scenario: Only reading
- **WHEN** the member's role in the inbox is reader
- **THEN** Show images is offered and Always show images is not
