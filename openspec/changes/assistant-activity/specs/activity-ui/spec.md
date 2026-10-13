## ADDED Requirements

### Requirement: Activity in the shell
The app SHALL show "Activity" in the sidebar with the shortcut G then A
(`nav.activity`), its icon a Visor showing what Campbooks is doing in the
workspace: `thinking` while a session the member may see is running, `alert`
while one failed or stalled in the last hour, `asleep` when no AI role is
configured, otherwise `idle`. On phones the Visor SHALL sit in the top bar as
a button that opens Activity. The Visor SHALL announce changes of state to
assistive technology without stealing focus, and SHALL not move under
reduced motion.

#### Scenario: Mail arrives while the member reads
- **WHEN** a sync batch arrives while the member is in the inbox
- **THEN** the sidebar Visor turns to thinking until the session is done, without the page reloading

### Requirement: The session list is live
`/w/$workspaceId/activity` SHALL list the member's visible sessions newest
first, 30 at a time with infinite scroll, filtered by All, Running or Needs a
look, each with its Visor, a one-line summary in the member's language, its
time, duration, models and, while running, its progress. New sessions and
status changes SHALL appear without a reload, from the change feed, without
moving the member's selection; when the selection is not at the top, a
"N new" control SHALL bring the new ones into view. J and K SHALL move, Enter
SHALL open.

#### Scenario: A batch in progress
- **WHEN** a 12-message batch is being sorted
- **THEN** its row reads "Sorting 12 new messages in Work" with "7 of 12" updating as the model answers, and reads "Sorted 12 new messages in Work" when done

#### Scenario: Selection kept
- **WHEN** the member has the third row selected and a new session starts
- **THEN** the selection stays on the same session and a "1 new" control appears

### Requirement: The session timeline
A session SHALL open beside the list from 64rem and as its own screen below,
with a header (kind, status, start, duration, roles and models, tokens) and
a timeline grouped by subject. Each group SHALL show the message's subject
and sender from `conversations`, and each stage as a line in the member's
language: what it answered, how sure, and whether it was applied or is a
suggestion, or that it is waiting its turn, working, skipped or failed (with
a plain reason, never a provider message). Steps SHALL stream in while the
session runs, fetching only steps after the last one shown. Each group SHALL
offer Why? (the reasons with their evidence, translated by rule id) and Open
conversation (O). Esc SHALL go back to the list. Status changes SHALL be
announced politely.

#### Scenario: Watching the model work
- **WHEN** the member opens a running session
- **THEN** the message being read shows the Visor thinking, finished messages show "Model: Primary, 97% sure. Not applied: answers are suggestions for now", and waiting ones show "Waiting its turn"

#### Scenario: Why
- **WHEN** the member opens Why? on a newsletter's group
- **THEN** it lists "It carries a mailing-list header (List-Id)" from the stored reasons

#### Scenario: A message the member cannot open
- **WHEN** the session's subjects include a message that was deleted
- **THEN** its group says the message is no longer available and the rest of the timeline is shown

### Requirement: Link from the reader
The reader's Why? SHALL offer "See what Campbooks did", opening the session
about the conversation's newest inbound message when one is on record.

#### Scenario: From a conversation
- **WHEN** the member opens Why? in the reader and chooses "See what Campbooks did"
- **THEN** Activity opens on the triage session of that message with its group in view

### Requirement: States, languages and sizes
The page SHALL have an empty state for no AI configured (Visor asleep, how to
configure the `decide` role) and for nothing yet, SHALL say "Sign in again to
see activity" when the session lacks `activity:read`, and SHALL ship every
string in en, pt-PT, es-ES and fr, calling the assistant "Campbooks". It
SHALL work without horizontal scrolling at 375px and on desktop, in light and
dark, with every shortcut shown from the registry.

#### Scenario: Phone
- **WHEN** the member opens Activity at 375px
- **THEN** the list fills the screen, a session opens as its own screen with Back, and nothing scrolls sideways
