# Tasks

## 1. Foundations

- [x] 1.1 `lib/api`: `signOut()`, `problemCode()`, `Schema<>` alias; verify with unit tests
- [x] 1.2 `lib/ui`: TextField, Select, ChoiceCards, FocusPage, Popover, WorkspaceMenu, ListRow, InlineConfirm, PhoneHeader, AccountButton, new icons; a story and a test each
- [x] 1.3 Shortcuts: `nav.settings` in the catalogue; module shortcuts handed to the registry through the manifest

## 2. Session and shell

- [x] 2.1 Session gate with landing, session-ended note and unreachable state; Tauri skips it
- [x] 2.2 `/` resolves the last or first workspace, or the first-run flow
- [x] 2.3 Workspace layout `/w/:workspaceId`: sidebar, switcher (⌘⇧W), account menu (theme, density, language, sign out), phone header and tab bar; accent per workspace
- [x] 2.4 Empty Home with the "Connect your mail" placeholder and visible shortcuts

## 3. Workspaces module

- [x] 3.1 Queries and mutations for list, create, members, roles, removal, invitations and accepting
- [x] 3.2 First-run flow (name → kind → done) with skip, back and signup-policy views
- [x] 3.3 Settings → Members: list, change role with undo, remove with inline confirm, invite, pending invitations
- [x] 3.4 Accept-invitation page
- [x] 3.5 `workspaces` i18n namespace in en, pt-PT, es-ES and fr

## 4. Verification

- [x] 4.1 Route and component tests with a stubbed API; stories for the new screens with axe
- [x] 4.2 Recording scenario `first-run` with intercepted BFF responses (demo data)
- [x] 4.3 CHANGELOG updated; client gates green; UI verified at 375px and desktop
