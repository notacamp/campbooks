## MODIFIED Requirements

### Requirement: A reply answers the message its draft was written for
Once a draft is asked for (a likely reply, your own words) or handed back
(an undo), the reply dialog SHALL keep answering that message: newer mail
arriving in the conversation SHALL never change what Send, Open in composer
or the quote answer. When that newer message is received mail (not the
member's own), the dialog SHALL then say "A newer message from <name>
arrived", with "Answer the newer one", which SHALL write the draft again for
the newer message (or, with no intent, quote it instead), never reusing a
draft saved for the older one. A draft already saved (a send that failed)
SHALL never be reused after Back either: the next save SHALL be a fresh
draft, and the saved one, still editable, SHALL be discarded. Open in composer SHALL open the composer on
the message the draft answers. When a send has no send time (no window to
take it back in), the toast SHALL offer no Undo. A reader without a
composer SHALL offer no Reply, Reply all or Forward, and R SHALL do nothing
there.

#### Scenario: Mail arrives while the draft is open
- **WHEN** the user has Scout's draft for Maya's message open and Leo's answer arrives in the conversation
- **THEN** the dialog says a newer message from Leo Brandt arrived, and Send replies to Maya's message with Maya's quoted

#### Scenario: Answer the newer one
- **WHEN** the user then chooses "Answer the newer one"
- **THEN** Scout writes the draft again for Leo's message, the quote is Leo's, and Send replies to it

#### Scenario: No window, no Undo
- **WHEN** a send comes back with no send time
- **THEN** the toast says "Sending to Maya Okafor" with no Undo

#### Scenario: Back after a failed send
- **WHEN** a send fails, and the user goes Back to the choices, picks another and sends
- **THEN** a fresh draft is sent, and the draft of the failed send is discarded

#### Scenario: Your own mail is not a newer message to answer
- **WHEN** the member's own sent mail lands in the conversation while the draft is open
- **THEN** the dialog says nothing of a newer message
