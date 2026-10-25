## MODIFIED Requirements

### Requirement: Calendar access and provider calendars in Settings
Settings → Calendar SHALL also list the member's provider calendars with sync
on or off, offer "Also add to" per kind, and, for each connection of theirs
that offers `calendar` (the connection's `offers`, as connectors answers it)
and does not hold it, "Add calendar access", which starts the connection's
consent for the `calendar` capability. The page SHALL NOT decide by provider
name which connections are offered it. When starting is refused it SHALL say
why in plain words: a refusal for a sign-in too old (`step_up_required`) SHALL
show a banner with "Sign in again", which signs in and comes back to Settings
→ Calendar; a provider this server cannot add it for, too many tries and any
other failure SHALL each have their own short sentence. It SHALL be in en,
pt-PT, es-ES and fr, and fit 375px.

#### Scenario: Adding calendar access
- **WHEN** the user presses "Add calendar access" next to their Gmail connection
- **THEN** the provider's consent opens for calendar access on that connection, and after it the Google calendars are listed

#### Scenario: Zoho when the server offers it
- **WHEN** a Zoho connection without calendar access offers `calendar`
- **THEN** Settings → Calendar shows "Add calendar access" next to it, and shows none while the connection does not offer it

#### Scenario: The sign-in is too old
- **WHEN** the user presses "Add calendar access" and the server answers 403 `step_up_required`
- **THEN** the page shows "Sign in again first" with a "Sign in again" button, not the generic failure, and the button returns to Settings → Calendar after signing in
