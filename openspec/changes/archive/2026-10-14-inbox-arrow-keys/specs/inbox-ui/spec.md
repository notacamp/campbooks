## ADDED Requirements

### Requirement: Moving through a stream with the arrow keys
In a stream's list, ↓ and ↑ SHALL move the keyboard cursor like J and K
(the first press lands on the first row) without opening the conversation,
and Enter SHALL open the row under the cursor. Moving past the last loaded row
SHALL load the next page, and the row the keyboard moves to SHALL be scrolled
into view. In an open conversation the arrow keys SHALL keep their browser
behaviour (scrolling it) while J and K step between conversations. Arrow keys
SHALL NOT be taken from a field, a radio group, a menu, a toolbar, a listbox,
a grid or an open dialog. The key list of the empty reader and the shortcut
sheet SHALL show both bindings ("J or ↓").

#### Scenario: Arrow down, then Enter
- **WHEN** the user presses ↓ three times, ↑ once, ↓ once and Enter in a stream
- **THEN** the cursor moves Q4, NDA, Ledger, NDA, Ledger and the Ledger conversation opens

#### Scenario: Arrows in an open conversation
- **WHEN** a conversation is open and the user presses ↓
- **THEN** the same conversation stays open and the browser scrolls it

#### Scenario: Arrows in a menu
- **WHEN** focus is in a menu, a field or a dialog and the user presses ↓
- **THEN** the list cursor does not move and the control handles the key
