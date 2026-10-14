# Tasks

## 0. Design gate

- [x] 0.1 Proposal, design (alternatives, decisions), spec deltas
- [x] 0.2 Visual prototype (desktop and 375px) published and linked in the PR
- [x] 0.3 Align with `teach-once` (#125): vocabulary, origins, per-inbox operations, `sender` role, port owned by #125, memory page shell owned here
- [x] 0.4 Owner correction: nothing taught goes into a decision model's prompt; questions without a deterministic reader dropped

## 1. `assistant` module

- [x] 1.1 Scaffold (`cargo xtask new-module assistant`), scopes, `architecture.toml`, composition with the AI gateway
- [x] 1.2 Migration `assistant__profiles` with forced RLS; verify with the RLS coverage test
- [x] 1.3 `get_profile` / `update_profile`, own-id only; verify with `#[sqlx::test]`, incl. an owner and an admin reading their own
- [x] 1.4 Purge on `member_left` and `workspace_deleting`
- [x] 1.5 Own-words sanitiser and refusal reasons; table tests incl. bidi and zero-width characters, URLs, pasted mail
- [x] 1.6 `compile_instruction`: context through `operations::invoke` as the member, strict-schema `complete` call, validation against the vocabulary and the member's data, `who` candidates, rate limit, 503 without a usable role; wiremock tests (good output, invented address, free-mail domain, forward, provider 500 with an echoing body, not configured, external refused)

## 2. `conversations`

- [x] 2.1 `list_senders` (readable inboxes only, bounded, no cursor, no subjects); access tests
- [x] 2.2 `list_labels` (readable inboxes only); access tests
- [x] 2.3 `InboxView.shared`, never naming the other members

## 3. Client (`clients/app/src/modules/assistant`)

- [x] 3.1 Home offer (Start / Not now)
- [x] 3.2 Question flow on `FocusPage`: senders, never-needs-you, proactivity, review grouped by inbox; shortcuts in the registry
- [x] 3.3 Rule sentence rendering from the vocabulary, in four languages
- [x] 3.4 Scout's memory page shell with the `rules` slot, Teach Scout composer (compile, `who` question, proposal card, honest unsupported/unclear answers), manual builder on 503
- [x] 3.5 Stories and tests for new `lib/ui` components; i18n parity en, pt-PT, es-ES, fr with diacritics

## 4. After #125 lands

- [x] 4.1 Merge `next`; implement the `MemberPreferences` port; turn on question 3; end-to-end test ("do it and tell me" → the next suggestion is active in a personal inbox)
- [x] 4.2 Wire `preview_rule` (counts and samples) and `create_rule` (Save, origin `onboarding` / `instruction`, one Undo)
- [ ] 4.3 Adopt #479's resource catalogue and `authorize` for `assistant.profile`, `list_senders` and `list_labels`; matrix tests

## 5. Finish

- [x] 5.1 OpenAPI regenerated, client types regenerated, `.sqlx` regenerated
- [x] 5.2 CHANGELOG updated
- [ ] 5.3 Gates green (Rust, client, OpenSpec validate)
- [x] 5.4 UI verified at 375px and desktop; recording of typing an instruction → proposed rule with preview → Save
- [ ] 5.5 Archive the change into `openspec/specs/`
