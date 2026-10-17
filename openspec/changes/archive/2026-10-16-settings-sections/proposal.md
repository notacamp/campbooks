# Proposal

## Why

Settings pages read as one undifferentiated column: section titles are 13px,
smaller than body text and the same size as field labels; Scout's memory
stacks "Your rules" over the rules list's own "Rules"; the page title is the
same size the section titles need to be. Nothing says where one section ends
and the next begins.

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/173.
The owner approved the design (desktop, phone and Ways to sign in mocks).

## What Changes

- **A shared Settings layout in the design system** (`clients/app/src/lib/ui`):
  `SettingsPage` (display-face title, lead, optional actions), `SettingsSection`
  (a raised card: title at 17px semibold, optional "Title · N" count, one-line
  description, body), `SettingsEmpty` (a quiet sunken panel: bold first line,
  guidance, optional button) and `PrivacyLine` (small line with a lock).
  Lists and fact lists inside a card drop their own panel, so there is never a
  card in a card. Tokens only; light, dark and 375px.
- **Every Settings and Admin page uses it:** Scout's memory, Ways to sign in
  (with Passkeys and Sign out everywhere), Members, Mailboxes, Apps and
  integrations, and the admin Signups, System, Jobs, People and Workspaces pages.
- **Scout's memory:** the field label "What should Scout do?" stays only for
  screen readers; the privacy line gets a lock; the rules list shows no heading of
  its own under "Your rules" (its empty state reads "No rules yet"); the answer is a
  "When Scout is sure" label and value.
- **Ways to sign in:** the ways in are an "Accounts" card with a count and
  "Add a way to sign in" at its bottom; the lead is "How you get into Campbooks.
  Keep at least two, so losing one never locks you out."; Sign out everywhere is
  its own card with a danger-outlined button (new `danger-outline` button variant).

Module: `clients/app` (lib/ui and the assistant, conversations, identity,
connectors, workspaces and system modules). No server change.

## Non-goals

- No change to behaviour, shortcuts, ids, routes or data.
- No new settings, features or server endpoints.
- No change to the pages' order or content, beyond short descriptions.
