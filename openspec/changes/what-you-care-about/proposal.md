# Proposal

## Why

The owner (2026-10-14): "we should ask users what they care about and so on
so we know how they want AI to act", and, for the rules this produces, "rules
should be translated into a smart rules system that become deterministic"
and "AI translates a user prompt into smart rules" (campbooks-cloud#127).

Today Scout knows nothing about the person it works for. The decide pass
sorts every inbox the same way, a newsletter asks the same question as a
client, and the only way to change that is to correct messages one at a
time. People should not have to write a prompt to fix it: Scout should ask
a few short questions, turn every precise answer into a deterministic rule
they can see and remove, and keep only what cannot be a rule as a short note
for the model.

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/121
(related: #73 onboarding, #125 teach once, #127 smart rules, #83
corrections).

## What Changes

- **A new `assistant` module** (`crates/modules/assistant`, schema
  `assistant`), pulled forward from milestone M4 with only what this needs:
  the member's **profile** (their answers) and the **compile** step. No
  chat yet. It depends on the kernel and `campbooks-ai` only, like every
  module.
- **Five optional questions**, one per view, every one skippable:
  1. what you use Campbooks for (work, personal life, both) and, in a few
     words, what you do;
  2. what matters most (pick up to three topics);
  3. whose mail should always reach you (people and domains);
  4. what never needs you (newsletters, automatic notifications, specific
     senders);
  5. when Scout is sure, should it ask first or act and tell you.
- **Every precise answer becomes a taught rule** (#125's model in
  `conversations`): "always reach me" is `sender_address`/`sender_domain`
  → `surface: always`; "never needs me" is `header list_id present`,
  `header auto_submitted present` or a sender → `surface: never`. Each is
  shown as a plain sentence with its match preview (count and examples)
  before it is saved, through `conversations.preview_rule` and
  `conversations.create_rule` (origin `onboarding`).
- **Teach Scout in your own words.** `assistant.compile_instruction` sends
  only the person's sentence and the names Scout needs (their inboxes'
  addresses, stream and label names) to the `complete` role, with a fixed
  instruction and a closed JSON schema, no tools. The output is validated
  against the taught-rule vocabulary; a person or role it cannot resolve
  ("my accountant") becomes a question to the person, never a guess. The
  person sees each proposed rule as a sentence with its preview and saves
  it (origin `instruction`, their words kept as `origin_text`). Once saved
  the rule runs with no AI. Without a `complete` role that may receive this
  content, Scout offers a manual rule builder instead.
- **A short, versioned preferences block** for the decide context's
  preferences slot (being added by the Laya decision-context fix): the
  answers that cannot be rules (what you use Campbooks for, what you do,
  what matters most, how proactive Scout should be, and at most three notes
  in the person's own words), rendered from templates, sanitised and capped
  at 600 characters, never quoting anyone's mail. It is a fallback the
  model reads; rules decide first.
- **Proactivity** changes one thing deterministically: whether rules Scout
  proposes from evidence (#125 `suggestion` origin) wait for the person
  (`ask_first`, the default) or start active with a notice and Undo
  (`act_then_tell`). It never lets a model answer act beyond the eval
  policy.
- **`conversations.list_senders`**: who writes to the caller's inboxes, with
  counts (received, replied, opened), for the question views' suggestions
  and for resolving a name in a sentence. Read-only, visible inboxes only.
- **A kernel port `MemberPreferences`**, implemented by `assistant`, that
  the conversations decide pass reads for the inbox owner's block.
- **UI:** the question flow as an onboarding step after the mailbox step
  (#73), the same flow reachable from Scout's memory, and, on #125's Scout's
  memory page, a "Teach Scout" composer, "About you" sentences (editable and
  removable) and "What Scout reads", in en, pt-PT, es-ES and fr.

## Capabilities

### New Capabilities

- `assistant`: the member's profile, the preferences block, and compiling
  language into taught rules.
- `assistant-ui`: the question flow.

### Modified Capabilities

- `conversations`: adds `conversations.list_senders` and passes the inbox
  owner's preferences block to the decide pass.
- `first-run-ui`: adds the "what you care about" step after the mailbox step.
- `memory-ui` (introduced by #125's `teach-once`): adds Teach Scout, About
  you and What Scout reads to Scout's memory.

## Impact

- New module crate and schema `assistant`; `architecture.toml` lists it.
- New kernel port trait `MemberPreferences` (kernel stays domain-free: the
  trait carries an opaque, already-rendered block).
- New scopes `assistant:read`, `assistant:write`; new resource type
  `assistant.profile` in the ADR 0017 catalogue.
- Depends on #125 (`teach-once`) for `conversations.preview_rule`,
  `create_rule`, `confirm_rule`, `list_rules` and the taught-rule
  vocabulary, and on the Laya decision-context change for the preferences
  slot. Neither is redefined here.
- Client: new `clients/app/src/modules/assistant`; first-run flow gains a
  step.
- No new crates expected; no new services; no change for self-hosters
  without a `complete` role beyond the manual builder.

## Non-goals

- The assistant chat (M4) and tool use. This change adds no conversation
  with Scout.
- The rule model, its storage, matching, precedence and bulk application
  (#125 owns them).
- How the preferences block is placed into the model's input (the Laya
  decision-context change owns the envelope and its version).
- Rules over calendars, documents or money; rules that send, forward,
  reply or delete. The compile step can only produce the taught-rule
  vocabulary.
- Promoting repeated model or k-NN confirmations into rules (detection is
  #125/#83); this change only decides how such proposals are confirmed.
- Workspace-wide "about this business" context shared by all members.
