## Purpose

What each member tells Scout about how they want it to act: a small,
member-private profile, and the step that turns the member's own sentence
into taught rules they preview and save.

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
Each member SHALL have at most one profile per workspace holding a
proactivity (`ask_first` or `act_then_tell`, default `ask_first`), when the
questions were last answered or put off (`asked_at`), and a revision that
grows on every change. `GET …/assistant/profile`
(`assistant.get_profile`, `assistant:read`) SHALL return the caller's own
profile, or the defaults when none exists. `PUT …/assistant/profile`
(`assistant.update_profile`, `assistant:write`) SHALL replace it and be its
own undo. A value outside the closed set SHALL be refused with 400.

#### Scenario: Defaults
- **WHEN** a member who never answered reads their profile
- **THEN** it says `proactivity` = `ask_first`, `asked_at` empty and revision 0

#### Scenario: Putting the questions off
- **WHEN** a member saves the profile with `asked_at` set and nothing else changed
- **THEN** the profile keeps `ask_first`, records `asked_at` and its revision grows by one

### Requirement: Profiles are private to their member
A profile SHALL be readable and writable by its member only: no workspace
role SHALL grant anything on another member's profile, client tokens SHALL
get none, and no operation SHALL take another member's id. Every table of
the `assistant` schema SHALL be isolated per workspace by forced row-level
security. A member's profile SHALL be deleted when `workspaces.member_left`
names them, and every profile of a workspace when
`workspaces.workspace_deleting` is published.

#### Scenario: An owner and an admin
- **WHEN** Marta chose "Do it and tell me", and the workspace's owner and an admin read their profiles
- **THEN** each gets their own profile, and nothing they can call returns Marta's

#### Scenario: Leaving
- **WHEN** Marta leaves the workspace
- **THEN** her profile no longer exists, and rejoining starts from the defaults

### Requirement: Proactivity for Scout's own rule proposals
The module SHALL implement the kernel's `MemberPreferences` port, answering
`acts_without_asking` as true only for a member whose profile says
`act_then_tell`.

#### Scenario: Tell me after
- **WHEN** a member chose "Do it and tell me"
- **THEN** the port answers true for them and false for every other member

#### Scenario: Scout's next proposal in a personal inbox
- **WHEN** a member who chose "Do it and tell me" gets a rule Scout proposes from their evidence (origin `suggestion`) in an inbox only they use
- **THEN** it is created active, with `acted_without_asking`, where with "Ask me first" it waited as proposed

### Requirement: Own words are sanitised
The instruction a member writes SHALL lose control, bidirectional and
zero-width characters, have runs of whitespace collapsed to one space and
be trimmed before it is checked or used. Text that is empty, longer than
300 characters, holds a URL, or looks pasted from mail (header lines,
quote markers, several lines, more than two addresses) SHALL be refused
with 400 and the code `instruction_empty`, `instruction_too_long`,
`instruction_has_link` or `instruction_pasted_mail`, without calling any
model.

#### Scenario: A hidden direction override
- **WHEN** an instruction contains a right-to-left override and a zero-width space
- **THEN** both are removed before the model sees it

#### Scenario: Pasted mail
- **WHEN** a member pastes "From: Ana <ana@example.com>\nSubject: Invoice" as an instruction
- **THEN** the response is 400 `instruction_pasted_mail` and no model is called

### Requirement: Compiling an instruction into taught rules
`POST …/assistant/compile` (`assistant.compile_instruction`,
`assistant:write`, not mutating) SHALL send the `complete` role, through
the provider that may receive content, only one JSON object with the
sanitised sentence, the member's own inbox addresses, the stream keys and
the member's label names (read as the member through
`conversations.list_inboxes` and `conversations.list_labels`), with a fixed
instruction, a strict JSON schema and no tools. It SHALL keep a proposed
rule only when every condition field is one of `sender_address`,
`sender_domain`, `subject_contains`, `header_present`, `label`, `stream`;
every action one of `move_to_stream`, `archive`, `add_label`, `mark_read`,
`needs_reply`, `asks_action`, `surface`, at most one per slot; every
address or domain appears in the sentence or is the member's own; no
domain is a free-mail domain; every subject word appears in the sentence;
every label is the member's and every stream one of the four; with 1 to 4
conditions or a `who` phrase from the sentence, 1 to 4 actions and at most
3 rules. A `who` phrase SHALL come back with up to 5 candidates from
`conversations.list_senders` and SHALL never be resolved by the model.
The answer SHALL say `rules`, `unsupported` (with the reason `sends_mail`,
`deletes_mail`, `not_about_mail` or `too_vague`) or `unclear`, and SHALL
store nothing. It SHALL allow 20 calls per member per hour (429), and
answer 503 with the AI gateway's reason, calling nothing, when the
`complete` role is not configured or may not receive content.

#### Scenario: Anything from my accountant
- **WHEN** a member writes "anything from my accountant always matters" and the model returns one rule with `who` = "my accountant" and action `surface: always`
- **THEN** the answer holds that proposal with candidates whose name, address or domain matches "accountant", ranked by how often the member replied, and nothing is saved

#### Scenario: An address in the sentence
- **WHEN** a member writes "always show mail from rita@silva-contas.example"
- **THEN** the answer holds `sender_address` = `rita@silva-contas.example` → `surface: always`

#### Scenario: An invented address
- **WHEN** the model returns a condition on `boss@corp.example`, which is not in the sentence nor the member's
- **THEN** that rule is dropped, and with nothing left the outcome is `unclear`

#### Scenario: Forwarding
- **WHEN** the model answers `unsupported` with reason `sends_mail` for "forward invoices to my accountant"
- **THEN** the answer says `unsupported` / `sends_mail` and holds no rule that sends anything

#### Scenario: No language model
- **WHEN** the `complete` role is not configured
- **THEN** the response is 503 with code `ai_role_not_configured` and no model is called

#### Scenario: An external provider without the opt-in
- **WHEN** the `complete` role points off the instance's network without `CAMPBOOKS_AI__COMPLETE__EXTERNAL_CONTENT=allow`
- **THEN** the response is 503 with code `ai_external_content_refused` and nothing is sent

### Requirement: Nothing personal in telemetry or logs
Spans, logs and metrics of this module SHALL never hold the member's text,
an address, a label name, a model response or a provider's error body;
compile SHALL record only its outcome, latency and counts.

#### Scenario: A failed call
- **WHEN** the provider answers 500 with a body echoing the request
- **THEN** the response is a problem without that body, and no log line holds it
