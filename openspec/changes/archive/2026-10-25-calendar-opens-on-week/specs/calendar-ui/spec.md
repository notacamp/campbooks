## MODIFIED Requirements

### Requirement: The Calendar place
Calendar SHALL be a place next to Home and Inbox in the sidebar and in the
phone tab bar, opened with G then S (`nav.schedule`, already in the shortcut catalogue). It SHALL open at today on Week, or on Agenda
below the phone breakpoint (48rem) when the address names no view, and a view in the
address SHALL always win. It SHALL offer Agenda, Week and Month, switched with 1, 2 and 3. T SHALL go to today,
J and K to the next and previous period, and N then E (N W, a new workspace,
is global and would hide a lone N) SHALL open a new event. Every
shortcut SHALL show its key where the action is, as the rest of the app
does, and be listed in the shortcuts sheet. Times SHALL be shown in the
member's time zone.

#### Scenario: Open the Calendar from the keyboard
- **WHEN** the user presses G then S
- **THEN** the Calendar opens on Week at today on a wide screen, or on Agenda at today below the phone breakpoint, marked as the current place in the sidebar

#### Scenario: Wide screen default
- **WHEN** the member opens the Calendar at desktop width with no view in the address
- **THEN** Week shows, and the address names no view

#### Scenario: Phone default
- **WHEN** the member opens the Calendar at 375px with no view in the address
- **THEN** Agenda shows, and the address names no view

#### Scenario: A view kept in the address
- **WHEN** the member picks Agenda on a desktop (or Week on a phone) and reloads
- **THEN** the address holds `?view=agenda` (or `?view=week`) and the same view shows again

#### Scenario: Next week
- **WHEN** the user is on Week and presses J
- **THEN** the next week shows
