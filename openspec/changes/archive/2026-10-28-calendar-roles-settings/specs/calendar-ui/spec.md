## MODIFIED Requirements

### Requirement: Calendar access and provider calendars in Settings
Settings → Calendar SHALL list the member's calendars in three groups, each
with a one-line description: **Mine** ("Make you busy, hold your tasks, and
send their invitations to Home"), **Following** ("Shown only") and **Off**
("Hidden"); a group with no calendar SHALL not show. Each provider
calendar SHALL have a three-way control (Mine / Following / Off) and its
colour; the Campbooks calendar SHALL show as Mine with no control. A
calendar the provider says the member does not own SHALL say "Shared with
you". Changing a role SHALL move the calendar to its group at once and offer
Undo. The page SHALL also offer "Also add to" per kind, and, for each
connection of theirs that offers `calendar` (the connection's `offers`, as
connectors answers it) and does not hold it, "Add calendar access", which
starts the connection's consent for the `calendar` capability. The page
SHALL NOT decide by provider name which connections are offered it. When
starting is refused it SHALL say why in plain words: a refusal for a sign-in
too old (`step_up_required`) SHALL show a banner with "Sign in again", which
signs in and comes back to Settings → Calendar; a provider this server cannot
add it for, too many tries and any other failure SHALL each have their own
short sentence. It SHALL be in en, pt-PT, es-ES and fr, and fit 375px.

#### Scenario: Adding calendar access
- **WHEN** the user presses "Add calendar access" next to their Gmail connection
- **THEN** the provider's consent opens for calendar access on that connection, and after it the Google calendars are listed

#### Scenario: Following a colleague's calendar
- **WHEN** the user sets "Marta" (shared with them) from Off to Following
- **THEN** "Marta" moves under Following, says "Shared with you", and its events appear on the Calendar

#### Scenario: Making a calendar theirs
- **WHEN** the user sets "Family" from Following to Mine
- **THEN** "Family" moves under Mine, and Undo puts it back under Following

#### Scenario: Zoho when the server offers it
- **WHEN** a Zoho connection without calendar access offers `calendar`
- **THEN** Settings → Calendar shows "Add calendar access" next to it, and shows none while the connection does not offer it

#### Scenario: The sign-in is too old
- **WHEN** the user presses "Add calendar access" and the server answers 403 `step_up_required`
- **THEN** the page shows "Sign in again first" with a "Sign in again" button, not the generic failure, and the button returns to Settings → Calendar after signing in

## ADDED Requirements

### Requirement: Only the member's own calendars take their time
Every place that puts something on one of the member's calendars SHALL offer
only their own (role Mine) calendars that can be written: the calendar for
Hold time, "Add to my calendar", and "Also add to" in Settings. A calendar
set to Following or Off after it was chosen for "Also add to" SHALL show the
choice as unset.

#### Scenario: Holding time
- **WHEN** the user holds time for a task and follows the writable calendar "Marta"
- **THEN** the calendar choice lists their Campbooks calendar and their own writable calendars, and not "Marta"
