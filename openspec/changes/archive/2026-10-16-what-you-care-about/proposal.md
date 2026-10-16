# Proposal

## Why

The owner (2026-10-14): "we should ask users what they care about and so on
so we know how they want AI to act", then "rules should be translated into
a smart rules system that become deterministic" and "AI translates a user
prompt into smart rules" (campbooks-cloud#127). Nothing the person teaches
goes into a decision model's prompt for now.

Today Scout knows nothing about the person it works for: a newsletter asks
the same question as a client, and the only way to change that is to
correct messages one at a time. Scout should ask a few short questions,
turn every answer into a deterministic rule the person can see and remove,
and turn a sentence the person types into the same kind of rule, with a
preview, before anything is saved.

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/121
(related: #73 onboarding, #125 teach once, #127 smart rules).

## What Changes

- **A new `assistant` module** (`crates/modules/assistant`, schema
  `assistant`), pulled forward from milestone M4 with only what this needs:
  the member's **profile** (proactivity and when the questions were
  answered) and the **compile** step. No chat. It depends on the kernel
  and `campbooks-ai` only.
- **Three optional questions**, one per view, every one skippable: whose
  mail should always reach you; what never needs you; when Scout is sure,
  should it ask first or act and tell you. The first two become taught
  rules (#125's model): `sender_address`/`sender_domain` → `surface:
  always`; `header list_id present`, `header auto_submitted present` or a
  sender → `surface: never`. The third is read by #125, through the kernel
  port it adds, when Scout proposes a rule from the member's evidence.
- **Teach Scout in your own words.** `assistant.compile_instruction` sends
  only the person's sentence, their own inbox addresses, the stream keys and
  their label names to the `complete` role, with a fixed instruction, a
  strict JSON schema and no tools. The server validates the output against
  the closed taught-rule vocabulary and the person's own data; a person
  named in the sentence ("my accountant") becomes a question with
  candidates, never a guess. The person previews and saves (origin
  `instruction`); the saved rule runs with no AI. A sentence that can't be
  a rule gets an honest answer. Without a usable `complete` role, a manual
  rule builder.
- **`conversations.list_senders`** and **`conversations.list_labels`**
  (read-only, readable inboxes only) for suggestions and validation, and
  **`InboxView.shared`** for marking rules "For everyone".
- **UI**: a "Tell Scout what you care about" offer on Home, the question
  flow, and the Scout's memory page shell at
  `/w/{id}/settings/memory` with Teach Scout, the manual builder and a slot
  #125 fills with the rules list, in en, pt-PT, es-ES and fr.

## Capabilities

### New Capabilities

- `assistant`: the member's profile and compiling language into taught
  rules.
- `assistant-ui`: the question flow and its offer on Home.
- `memory-ui`: the Scout's memory page with Teach Scout and the manual
  builder (#125 adds the rules list to the same capability).

### Modified Capabilities

- `conversations`: adds `list_senders`, `list_labels` and
  `InboxView.shared`.

## Impact

- New module crate and schema `assistant`; `architecture.toml` lists it.
- New scopes `assistant:read`, `assistant:write`; new resource type
  `assistant.profile`.
- Depends on #125 (`teach-once`) for `conversations.preview_rule`,
  `create_rule`, `list_rules`, the taught-rule vocabulary and the
  `MemberPreferences` port. Neither is redefined here.
- Client: new `clients/app/src/modules/assistant`.
- No new crates; no new services; nothing changes for self-hosters without
  a `complete` role beyond the manual builder.

## Non-goals

- Any text in a decision model's prompt from what the person teaches.
- The assistant chat (M4) and tool use.
- The rule model, storage, matching, precedence and bulk application
  (#125 owns them).
- Rules over calendars, documents or money; rules that send, forward,
  reply or delete.
- Detecting rules to propose from repeated corrections (#125/#83).
