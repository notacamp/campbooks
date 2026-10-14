## Purpose

What each member tells Scout about how they want it to act: a small profile
of answers, a short versioned preferences block for the model's fallback,
and the step that turns the member's own sentence into taught rules they
confirm.

## ADDED Requirements

### Requirement: Assistant scopes
The `assistant` module SHALL declare the scopes `assistant:read` and
`assistant:write`, and every endpoint it adds SHALL require one of them.

#### Scenario: Anonymous caller
- **WHEN** a client calls `GET /api/v1/w/{workspace_id}/assistant/profile` without an access token
- **THEN** the response is 401 as `application/problem+json`

#### Scenario: Token without the write scope
- **WHEN** a client with only `assistant:read` updates the profile
- **THEN** the response is 403 as `application/problem+json`

### Requirement: The member's profile
Each member SHALL have at most one profile per workspace holding: what they
use Campbooks for (`work`, `personal` or `both`), what they do in their own
words (at most 80 characters), up to three topics from a closed set
(`clients`, `invoices_payments`, `suppliers`, `appointments`, `team`,
`taxes`, `family`, `school`), a proactivity (`ask_first` or
`act_then_tell`, default `ask_first`), up to three notes in their own words
(at most 200 characters each), when the questions were last answered or
skipped, and a revision that grows on every change.
`GET …/assistant/profile` (`assistant.get_profile`, `assistant:read`)
SHALL return the caller's own profile, or the defaults when none exists.
`PUT …/assistant/profile` (`assistant.update_profile`, `assistant:write`)
SHALL replace it and be its own undo. `POST …/assistant/profile/notes`
(`assistant.add_note`) and `DELETE …/assistant/profile/notes/{note_id}`
(`assistant.remove_note`) SHALL be an undo pair. `DELETE
…/assistant/profile` (`assistant.reset_profile`) SHALL remove it. A value
outside the closed sets SHALL be refused with 400 `profile_invalid`.

#### Scenario: Defaults
- **WHEN** a member of a Business workspace who never answered reads their profile
- **THEN** it says `uses` = `work`, no topics, `proactivity` = `ask_first`, no notes and `asked_at` empty

#### Scenario: A fourth topic
- **WHEN** a member saves four topics
- **THEN** the response is 400 with code `profile_invalid` and the profile is unchanged

### Requirement: Profiles are private to their member
The resource type `assistant.profile` SHALL be owned by its member, who
alone holds `read`, `write` and `delete`; no workspace role SHALL be
granted anything on it, and client tokens SHALL hold `read` only. Every
table of the `assistant` schema SHALL be isolated per workspace by forced
row-level security. A member's profile SHALL be deleted when
`workspaces.member_left` names them, and every profile of a workspace when
`workspaces.workspace_deleting` is published.

#### Scenario: An owner looks
- **WHEN** a workspace owner calls `assistant.get_profile`
- **THEN** they get their own profile, and no operation returns another member's

#### Scenario: Leaving
- **WHEN** Marta leaves the workspace
- **THEN** her profile no longer exists, and rejoining starts from the defaults

### Requirement: Sanitised own words
Text the member writes (`does`, notes, instructions) SHALL be normalised to
NFC, stripped of control and format characters (including bidirectional
overrides and zero-width characters), with runs of whitespace and newlines
collapsed to one space and trimmed, before it is checked or stored. Text
that is empty, longer than its limit, contains a URL, or looks pasted from
mail (header lines, quote markers, several lines, several addresses) SHALL
be refused with 400 `own_words_invalid` and a `reason` of `empty`,
`too_long`, `url`, or `pasted_mail`.

#### Scenario: A hidden direction override
- **WHEN** a note contains a right-to-left override and a zero-width space
- **THEN** both are removed and the stored note is what the member sees

#### Scenario: Pasted mail
- **WHEN** a member pastes "From: Ana <ana@example.com>\nSubject: Invoice" as a note
- **THEN** the response is 400 `own_words_invalid` with reason `pasted_mail`

### Requirement: The preferences block
The module SHALL implement the kernel port `MemberPreferences`, returning
for a member a block with the schema name `member_preferences.v1`, the
profile's revision, and English sentences rendered from fixed templates for
each answer given, with the member's own words only inside quoted JSON
strings labelled as their words. The sentences together SHALL be at most
600 characters: template sentences first, then notes newest first while
they fit. The block SHALL never contain an address, a person's name other
than the workspace's, a subject or any message text. A member with no
answers SHALL get no block.

#### Scenario: Answers to sentences
- **WHEN** a member of "Atelier Sousa" answered work, "Architect, small studio", clients and invoices, and ask first
- **THEN** the block's sentences say they use Campbooks for work at Atelier Sousa, quote "Architect, small studio" as their own words, list clients and invoices and payments, and say they want to be asked first

#### Scenario: Over budget
- **WHEN** template sentences take 420 characters and three notes of 150 characters exist
- **THEN** the block holds the templates and only the newest note

### Requirement: Compiling an instruction into taught rules
`POST …/assistant/instructions/compile` (`assistant.compile_instruction`,
`assistant:write`, not mutating) SHALL take the member's sentence (at most
300 characters, sanitised as their own words) and return proposals in the
request shape of `conversations.create_rule` with origin `instruction`
and the sentence as `origin_text`, each with its preview from
`conversations.preview_rule`, plus clarifications and, when the sentence
is a preference rather than a rule, a proposed note. It SHALL send the
`complete` role only the sentence, the member's own inbox addresses, the
stream keys and the member's label names, as one JSON object inside a fixed
instruction, with a strict JSON schema and no tools, through the provider
that may receive content. It SHALL keep a condition or action only when it
is in the taught-rule vocabulary, every label and stream exists, every
address or domain appears in the sentence, no domain is a free-mail domain,
and at most three rules are proposed. A phrase naming someone ("my
accountant") SHALL be returned as a clarification with candidates from
`conversations.list_senders`, never resolved by the model or picked
without the member. Nothing SHALL be saved by this operation. It SHALL be
limited to 20 calls per member per hour (429 `rate_limited`), and SHALL
answer 409 `compile_unavailable` when the `complete` role is not
configured or may not receive content.

#### Scenario: Anything from my accountant
- **WHEN** a member writes "anything from my accountant always matters" and the model returns a rule with `who` = "my accountant" and action `surface: always`
- **THEN** the response holds one clarification for "my accountant" whose first candidate is the sender the member replied to most whose name or domain matches, and no saved rule exists

#### Scenario: A resolved rule with its preview
- **WHEN** the member then compiles "always show mail from rita@silva-contas.example"
- **THEN** the response holds one proposal with `sender_address is rita@silva-contas.example` → `surface: always`, origin `instruction`, and a preview of 41 matching emails with samples

#### Scenario: An invented address
- **WHEN** the model returns a condition on `boss@corp.example` that is not in the sentence
- **THEN** that rule is dropped and a clarification asks who is meant

#### Scenario: Forwarding
- **WHEN** a member writes "forward invoices to my accountant"
- **THEN** no rule forwards anything, and the response says Scout cannot forward mail and offers the closest rule it can make

#### Scenario: No language model
- **WHEN** the `complete` role is not configured
- **THEN** the response is 409 with code `compile_unavailable` and no model is called

### Requirement: Answers become taught rules
Saving the question flow SHALL create, for each inbox where the member
holds `sender` and that is on (their own inboxes by default; an inbox shared
with others only when the member switched it on), one rule per precise
answer with origin `onboarding` and one
`teaching_id` per answer across inboxes: a sender to always see as
`sender_address is X` (or `sender_domain is D`) → `surface: always`;
newsletters as `header list_id present` → `surface: never`; automatic
notifications as `header auto_submitted present` → `surface: never`; a
sender that never needs the member as `sender_address is X` → `surface:
never`. Inboxes where the member is only a reader SHALL get no rule.

#### Scenario: A shared inbox left off
- **WHEN** a member picks newsletters and saves without switching on the shared info@ inbox
- **THEN** the rule exists in their own inbox only, and info@ is unchanged for everyone

#### Scenario: Always see my accountant
- **WHEN** a member manager of one inbox and reader of a shared one picks Rita Gomes under "always reach you" and saves
- **THEN** one active rule `sender_address is rita@silva-contas.example` → `surface: always` exists in the first inbox only, with origin `onboarding`

### Requirement: Proactivity for proposed rules
When a rule is proposed from evidence for an inbox (origin `suggestion`),
it SHALL be created `proposed` when the inbox owner's proactivity is
`ask_first`, and `active` with a notice and Undo when it is
`act_then_tell`. Proactivity SHALL NOT change which model answers the
decide policy accepts.

#### Scenario: Tell me after
- **WHEN** the inbox owner chose "Do it and tell me" and a suggestion rule is made
- **THEN** it is active and logged with Undo, and model answers still act only where the policy allows

### Requirement: Nothing personal in telemetry
Spans, logs and metrics of this module SHALL never hold the member's text,
an address, a label name or a model response; compile SHALL record only
its outcome kind, latency and counts.

#### Scenario: Compile logged
- **WHEN** a compile succeeds
- **THEN** the telemetry holds the outcome `rules`, the latency and the number of proposals, and not the sentence
