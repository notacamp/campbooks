# Tasks

## 0. Design gate

- [x] 0.1 Proposal, design (alternatives, owner questions), spec deltas
- [x] 0.2 Visual prototype (desktop and 375px) published and linked in the PR
- [x] 0.3 Align with `teach-once` (#125)'s pushed draft: vocabulary, `origin` values, per-inbox `preview_rule`/`create_rule`, `sender` role, Scout's memory page owned by its `memory-ui`
- [ ] 0.4 Align with the decision-context change: where the preferences slot sits and how its revision is recorded
- [ ] 0.5 Agree with #125 that rules it proposes (origins `suggestion`, `correction`) read `acts_without_asking` from the port

## 1. Kernel

- [ ] 1.1 `MemberPreferences` port trait and `PreferencesBlock` (opaque schema, revision, sentences) installed in `AppState`; a no-op default; verify with a unit test
- [ ] 1.2 Shared own-words sanitiser (NFC, Cc/Cf removal, whitespace collapse, URL and pasted-mail detection) in `kernel::text`; verify with table tests incl. bidi overrides and zero-width characters

## 2. `assistant` module

- [ ] 2.1 `cargo xtask new-module assistant`; scopes, `architecture.toml`, resource type `assistant.profile` (owner only, no role grants, tokens read)
- [ ] 2.2 Migration `assistant__profiles` with forced RLS; verify with the RLS coverage test
- [ ] 2.3 Slices `get_profile`, `update_profile`, `add_note`, `remove_note`, `reset_profile` with validation and undo pairs; verify with `#[sqlx::test]` incl. another member and an owner (404 / own only)
- [ ] 2.4 Purge on `member_left` and `workspace_deleting`; verify replay
- [ ] 2.5 Preferences block renderer (templates, 600-char budget, quoting) and the port implementation; verify the scenarios of "The preferences block"
- [ ] 2.6 Answers → rules mapping (onboarding) calling `conversations.preview_rule` / `create_rule` through `kernel::operations::invoke`; verify readers get no rule
- [ ] 2.7 `compile_instruction`: context gathering via invoke, `complete` call with strict schema through `provider_for_content`, validation against the vocabulary and the inputs, `who` resolution via `list_senders`, previews, rate limit, 409 fallback; verify with wiremock (good output, invented address, extra fields, forward request, not configured, external refused)
- [ ] 2.8 Compile eval set: 40 instructions per locale with expected rules or clarifications; `cargo xtask eval` entry, not a CI gate

## 3. `conversations`

- [ ] 3.1 `list_senders` slice (visible inboxes only, `q`, sorts, cursor); verify another member's inbox never leaks
- [ ] 3.2 Decide pass reads `MemberPreferences` for the inbox owner and records schema + revision with each decision

## 4. Client (`clients/app/src/modules/assistant`)

- [ ] 4.1 Question flow on `FocusPage` + `ChoiceCards` / chips, progress, shortcuts in the registry (`assistant.next`, `assistant.skip`, `assistant.later`, `assistant.back`); stories and tests
- [ ] 4.2 Rule preview rows (sentence, count, samples) as a `lib/ui` component with story and test
- [ ] 4.3 Review view, Save with one Undo
- [ ] 4.4 Teach Scout composer: compile, clarifications, proposals, manual builder fallback
- [ ] 4.5 Scout's memory: About you, Your rules, What Scout reads
- [ ] 4.6 First-run step after the mailbox step; Home offer for 7 days
- [ ] 4.7 Why? names the taught rule
- [ ] 4.8 i18n en, pt-PT, es-ES, fr with diacritics

## 5. Finish

- [ ] 5.1 OpenAPI regenerated, client types regenerated
- [ ] 5.2 CHANGELOG updated
- [ ] 5.3 Gates green (Rust, client, OpenSpec validate)
- [ ] 5.4 UI verified at 375px and desktop; recording of typing an instruction → proposed rule with preview → Save
- [ ] 5.5 Archive the change into `openspec/specs/`
