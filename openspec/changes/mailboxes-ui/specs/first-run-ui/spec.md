## MODIFIED Requirements

### Requirement: Workspace shell
Inside a workspace the client SHALL show a sidebar (from 48rem) or a phone
header and tab bar (below 48rem) with the workspace switcher, Home, Inbox
with its streams, and Settings, and an account menu with the user's name and
email, theme (system, light, dark), density, language and Sign out. Inside
Settings, its sections (Members, Mailboxes, Ways to sign in) SHALL be listed
under Settings in the sidebar and as chips under the phone header. The
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

#### Scenario: Settings sections on a phone
- **WHEN** the user opens Settings at 375px
- **THEN** Members, Mailboxes and Ways to sign in are offered as chips, the open one pressed

### Requirement: Empty Home
A workspace's Home SHALL say that nothing needs the user yet and show the
keyboard shortcuts available on the page. Its mail section SHALL offer the
person's held mailbox grant when there is one, else name the mailboxes
connected to the workspace that the person can see with their health and a
link to Settings → Mailboxes, else offer "Connect your mail", which opens
Settings → Mailboxes.

#### Scenario: New workspace
- **WHEN** a user with no held mailbox and no connection opens a workspace's Home
- **THEN** "Nothing needs you yet" is shown and "Connect your mail" opens Settings → Mailboxes

#### Scenario: Mailbox connected
- **WHEN** `connectors.list_connections` returns a connection for `ana@example.com`
- **THEN** Home says that `ana@example.com` is syncing and links to Settings → Mailboxes

### Requirement: Held mailbox offer on Home
While the person has a held grant, Home SHALL offer it with the address,
until when it waits, Connect (`identity.adopt_held_grant` for the open
workspace), "Don't connect it" (`identity.discard_held_grant` after an
inline confirmation) and "Connect a different mailbox", which opens
Settings → Mailboxes. After Connect, Home SHALL name the connected mailbox.

#### Scenario: Forget the mailbox
- **WHEN** the person chooses "Don't connect it" and confirms
- **THEN** `identity.discard_held_grant` is called and the offer is gone

#### Scenario: A different mailbox
- **WHEN** the person chooses "Connect a different mailbox"
- **THEN** Settings → Mailboxes opens and the held grant still waits
