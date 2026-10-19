## ADDED Requirements

### Requirement: A reply answers the message its draft was written for
Once a draft is asked for (a likely reply, your own words) or handed back
(an undo), the reply dialog SHALL keep answering that message: newer mail
arriving in the conversation SHALL never change what Send, Open in composer
or the quote answer. The dialog SHALL then say "A newer message from <name>
arrived", with "Answer the newer one", which SHALL write the draft again for
the newer message (or, with no intent, quote it instead), never reusing a
draft saved for the older one. Open in composer SHALL open the composer on
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

### Requirement: Dialog keys stay in the dialog
A dialog SHALL keep the focus inside it: when its focused control goes away
(a step replaces it) or the focus leaves for nowhere, the focus SHALL return
to the dialog, so Esc, its number keys and arrows keep working and plain
keys never reach the page behind it. It SHALL listen on itself only, never
on the whole document: a key the page itself receives SHALL be the page's.

#### Scenario: A step replaces the focused control
- **WHEN** the focused button of a dialog is replaced by choices, and the user presses 2
- **THEN** the second choice is picked, and Esc closes the dialog
