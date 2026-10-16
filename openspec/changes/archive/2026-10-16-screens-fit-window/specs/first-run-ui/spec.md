# Delta for first-run-ui

## MODIFIED Requirements

### Requirement: Workspace shell
Inside a workspace the client SHALL show a sidebar (from 48rem) or a phone
header and tab bar (below 48rem) with the workspace switcher, Home, Inbox
with its streams, and Settings, and an account menu with the user's name and
email, theme (system, light, dark), density, language and Sign out. Inside
Settings, its sections (Members, Mailboxes, Ways to sign in) SHALL be listed
under Settings in the sidebar and as link chips under the phone header, the
open one marked as the current page. The
switcher SHALL open with ⌘⇧W (Ctrl+Shift+W elsewhere), list the user's
workspaces with their accent, mark the current one, and offer to create a
workspace. Opening a workspace SHALL apply its accent and remember it as the
last workspace on this device. From 64rem the shell SHALL be exactly the
window's height: the sidebar and every pane beside it (a list, a reader,
Home) scroll on their own, so no screen's content moves the sidebar or the
account menu. Scrollbars SHALL be thin and drawn from the design tokens (a
neutral thumb on no track, firmer while the pointer is over the pane) in
light and dark.

#### Scenario: Switch workspace from the keyboard
- **WHEN** the user presses ⌘⇧W and picks another workspace
- **THEN** the client navigates to that workspace and the document's accent becomes that workspace's accent

#### Scenario: Go to the inbox
- **WHEN** the user presses G then I
- **THEN** the client opens the workspace's Primary stream

#### Scenario: Settings sections on a phone
- **WHEN** the user opens Settings at 375px
- **THEN** Members, Mailboxes and Ways to sign in are offered as chips, the open one pressed

#### Scenario: A busy Home on desktop
- **WHEN** more things need the user than fit in a 1100px or 1440px window
- **THEN** Home scrolls inside its pane, the page is no taller than the window, and the sidebar with the account menu stays in place

#### Scenario: Scrollbars follow the theme
- **WHEN** a pane scrolls in light or dark
- **THEN** its scrollbar is thin, its thumb a neutral token colour of that theme, with no track
