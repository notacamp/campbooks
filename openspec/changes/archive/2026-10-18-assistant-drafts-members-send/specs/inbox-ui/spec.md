## MODIFIED Requirements

### Requirement: Sending only what the member was shown
The composer SHALL send, with every save and with Send, the
`content_token` of the content on screen: the one of the draft it opened
with, then of each save's answer or re-read it applies, never one from a
list or a read it does not show. When an autosave answers 409
`content_changed` (or `draft_changed`), it SHALL keep what the member
typed, read the draft again for its version, token and attachments, show
the notice once, and save nothing on its own: the next edit or Send saves
the typed text with the fresh token. When Send answers 409
`content_changed` it SHALL read the draft again and show it as it is now
(its recipients, Cc and Bcc included, its subject and text), with "This
changed since you opened it. Check it and send again." in all four locales,
and SHALL NOT send it again on its own. The message composer SHALL show Cc
and Bcc whenever the draft has any, also ones that arrive after it opened.
The reply dialog SHALL show every recipient the reply goes to and SHALL
NOT reuse a draft it did not create (one from the drafts list, or the one
an Undo handed back): it SHALL create a fresh reply draft with exactly those
recipients, no Cc or Bcc, its own quoted text and no attachments, discard
the draft it replaces while that one is editable (a `draft`, or a `failed`
send) and never one that is `scheduled` or `sending`, which only its Undo
takes back, and send with the token of that draft; it MAY save
again only the draft it created, with its token. On 409 `content_changed`
it SHALL say so and send nothing until the member sends again, in a fresh
draft. The forward dialog SHALL save into an untouched forward draft with
its token and start a fresh forward when that is refused.

#### Scenario: The draft changed since it was opened
- **WHEN** Sam presses Send and the server answers 409 `content_changed`
- **THEN** the composer shows the draft as the server holds it, a Bcc Sam never added included, with the notice, and nothing is sent until Sam presses Send again

#### Scenario: An autosave refused
- **WHEN** the composer's autosave answers 409 `content_changed`
- **THEN** the typed text stays, the notice shows once, nothing is saved again until Sam types or presses Send, and that save uses the token of the draft read again

#### Scenario: A reply dialog after an Undo
- **WHEN** the draft an Undo handed back holds a Bcc and an attachment the dialog does not show, and Sam sends from the dialog
- **THEN** a fresh draft with only the recipients shown is created and sent, and the old one is discarded

#### Scenario: Opened before the upgrade
- **WHEN** Sam types or presses Send in a composer opened before the upgrade
- **THEN** the notice shows once, Sam's text is still there, and the next save and Send work

#### Scenario: Replacing a draft whose send failed
- **WHEN** the draft an Undo or a failed send handed back is `failed`, and Sam opens Scout's text in the composer
- **THEN** a fresh reply draft is created and the failed one is discarded

#### Scenario: Replacing a draft on its way
- **WHEN** the draft handed back is `scheduled` (sent again elsewhere since)
- **THEN** a fresh reply draft is created and the scheduled one is left alone
