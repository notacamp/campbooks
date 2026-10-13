## MODIFIED Requirements

### Requirement: Workspace shell
Inside a workspace the client SHALL show a sidebar (from 48rem) or a phone
header and tab bar (below 48rem) with the workspace switcher, Home, Inbox
with its streams, and Settings, and an account menu with the user's name and
email, theme (system, light, dark), density, language and Sign out. The
switcher SHALL open with ⌘⇧W (Ctrl+Shift+W elsewhere), list the user's
workspaces with their accent, mark the current one, and offer to create a
workspace. Opening a workspace SHALL apply its accent and remember it as the
last workspace on this device.

#### Scenario: Switch workspace from the keyboard
- **WHEN** the user presses ⌘⇧W and picks another workspace
- **THEN** the client navigates to that workspace and the document's accent becomes that workspace's accent

#### Scenario: Go to the inbox
- **WHEN** the user presses G then I
- **THEN** the client opens the workspace's Primary stream
