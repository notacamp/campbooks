# Unified Action System — End-Goal Spec

> Status: **target spec** for incremental work by multiple agents. Two fixes already landed (marked ✅). The rest is the backlog.

## 1. The problem

A user can act on an email/thread from four surfaces — the **email UI** (single + bulk), the **Cmd+K palette**, **Scout** (AI), and **Skim** — but each surface was built independently. Today there are **five+ execution dispatchers** with overlapping, inconsistent tool sets:

| Dispatcher | Used by | File |
|---|---|---|
| `EmailToolsController#create` | Scout *suggested* actions (single thread) | `app/controllers/email_tools_controller.rb` |
| `Tools::Executor` | Scout *auto* actions (background) | `app/services/tools/executor.rb` |
| `EmailMessages::BulkController#dispatch_tool` | Bulk multi-select | `app/controllers/email_messages/bulk_controller.rb` |
| `EmailComposeController` | Reply / reply-all / forward / send | `app/controllers/email_compose_controller.rb` |
| `EmailMessageTagsController` | Manual tag/label by ID (handles both local and external/synced tags) | — |
| `Emails::SkimArchive` / `Emails::SkimRestore` | Skim | `app/services/emails/` |

The same intent runs through different code on different surfaces, so behavior diverges (e.g. **"delete" permanently destroys records** via `BulkDelete`, but Scout's **"trash" only moves to a folder** via `Tools::Trash`). And the AI can propose actions the user can't actually perform.

**The symptom that started this:** Scout suggested *"add tag: security"*, rendered as a bare `+ security` chip, and clicking it returned "Action failed." Root cause: Scout's prompt never listed the real tags, so it invented one; `Tools::AddTag` correctly refused (no such tag) — the same way the manual tag picker would. Three distinct failures in one click: ungrounded AI, unclear label, opaque error.

## 2. Vision & principles

1. **One canonical action registry.** Every email/thread action is defined once — id, label, argument schema, target (message vs thread), destructive?, required permission, execution, and which surfaces expose it. All surfaces consume the registry; none re-implement.
2. **Surface parity.** If an action is possible manually, it's possible via Cmd+K and Scout too (and via Skim where it makes sense), with identical execution, permissions, and result.
3. **Scout is grounded in real state.** The AI may only propose actions the user could actually perform — real tags, valid folders, sendable accounts. It never invents tags/labels/folders. Its tool list and its state (tags, folders) are generated *from the registry + live data*.
4. **One execution path per action.** Surfaces are thin adapters that call `Actions.run(:id, target:, args:, user:)`. No duplicated logic (today Skim's reply re-implements `EmailToolsController#send_reply`).
5. **Self-describing UI.** Every action button states verb + object ("Add tag: invoice", "Snooze until Fri 9am"), not a bare value. Failures are specific ("No tag named 'security' — create it in Settings → Tags first").
6. **Reconciled semantics.** One meaning per verb across surfaces. Decide and unify: `trash` (recoverable, move to Trash folder) vs `delete` (permanent). Single-message and bulk share the same definitions.

## 3. Canonical action catalog

Target set (union of everything found, de-duplicated and named consistently). `target`: what it acts on. `perm`: `read` (any reader) / `send` (sendable account) / `manage`.

| Action id | Label template | Target | Destructive | Perm | Execution (today) |
|---|---|---|---|---|---|
| `reply` / `reply_all` / `forward` | "Reply" / "Reply all" / "Forward" | message | no | send | `EmailComposeController` |
| `send_reply` | "Send reply" | message | no (sends mail) | send | `EmailToolsController#send_reply` |
| `draft_reply` | "Draft a reply" | thread | no | read | `Tools::DraftReply` (AI) |
| `save_draft` / `send_draft` / `discard_draft` | "Save draft" / "Send draft" / "Discard" | message | varies | send/read | `EmailToolsController` (private) |
| `archive` | "Archive" | thread | no (recoverable) | read | `Tools::Archive` / `Tools::BulkArchive` |
| `trash` | "Move to Trash" | thread | no (recoverable) | read | `Tools::Trash` |
| `delete` | "Delete permanently" | thread | **yes** | manage | `Tools::BulkDelete` (destroys DB rows) |
| `snooze` / `unsnooze` | "Snooze until …" / "Unsnooze" | thread | no | read | `Tools::Snooze` / `Tools::Unsnooze` |
| `mark_read` / `mark_unread` | "Mark read" / "Mark unread" | thread | no | read | `Tools::BulkMarkRead` / jobs |
| `add_tag` / `remove_tag` | "Add tag: X" / "Remove tag: X" | message/thread | no | read | `Tools::AddTag` / `Tools::RemoveTag` · `EmailMessageTagsController` (synced tags mirror to provider labels) |
| `move_to_folder` | "Move to: Folder" | thread | no | read | `Tools::BulkMoveToFolder` |
| `reclassify` | "Re-classify" | message(s) | no | read | `Tools::Reclassify` (AI) |
| `process_ai` | "Re-analyze with AI" | message(s) | no | read | `Tools::BulkProcessAi` |
| `dismiss_todo` | "Dismiss" | message | no | read | `EmailMessagesController#dismiss_todo` |
| `follow` / `unfollow` | "Follow" / "Following" | thread | no | read | `ThreadFollowsController` |
| `scout_chat` | "Ask Scout" | message(s) | no | read | `Tools::BulkScoutChat` |
| `undo` | "Undo" | last action | no | — | `Emails::SkimRestore` (archive only) |

> **Tag rule (applies everywhere):** `add_tag`/`remove_tag` only attach tags that already exist in the workspace — identical to the manual picker (which selects by tag id). Creating a tag is a separate, explicit action (Settings → Tags). Scout must be given the live list and forbidden from inventing names.

## 4. Target architecture

```
                       ┌────────────────────────────┐
  Email UI (single) ──▶│                            │
  Email UI (bulk)   ──▶│   EmailActions registry    │──▶ Tools::* services
  Cmd+K palette     ──▶│   .run(:id, target:, args:,│    (the single source of
  Scout (suggest)   ──▶│         user:)             │     execution logic)
  Scout (auto)      ──▶│                            │
  Skim              ──▶│   .available_for(surface,  │
                       │     target, user, state)   │
                       └────────────────────────────┘
```

- **`EmailActions` registry** (new, Ruby): each action declares `{ id, label:, target:, args:, destructive:, perm:, surfaces:, execute: }`. `execute` wraps the existing `Tools::*` service and returns one normalized result `{ success:, message:, turbo: [...] }`.
- **Thin adapters.** `EmailToolsController`, `BulkController`, the Cmd+K JS, `Tools::Executor`, and Skim all dispatch through `EmailActions.run`. They stop carrying their own `case tool` ladders.
- **Generated surfaces.** Cmd+K context commands, the bulk toolbar, and Scout's tool list are all *generated* from `EmailActions.available_for(surface, target, user, state)` — so adding an action once exposes it everywhere it's allowed.
- **Grounded Scout.** Scout's system prompt is built from the registry (the allowed tools) plus live state (the workspace's tags, the account's folders). `Ai::Configuration` / `EmailChatService` inject these. ✅ *(tags done — see §6)*

## 5. Surface parity matrix (target)

`UI`=manual button, `K`=Cmd+K, `S`=Scout (suggest/auto), `Sk`=Skim. `●` target, `·` n/a. Bold = **current gap** to close.

| Action | UI single | UI bulk | Cmd+K | Scout | Skim |
|---|---|---|---|---|---|
| reply / forward | ● | · | ● | ● | ● (**add to Skim via registry**) |
| archive | ● (**add single button**) | ● | ● | ● | ● |
| trash | ● (**add**) | ● | **● add** | ● | **● add** |
| delete (permanent) | · | ● | **● add (confirm)** | **● add (confirm)** | · |
| snooze / unsnooze | ● | ● | **● add** | ● | **● add** |
| mark read / unread | **● add single** | ● | **● add** | **● add tool** | **● add** |
| add/remove tag | ● | ● | ● (single only → **add bulk**) | ● ✅ | **● add** |
| move to folder | **● add single** | ● | ● | **● add tool** | **● add** |
| draft_reply (AI) | · | · | **● add** | ● | **● add** |
| reclassify | **● add** | **● add** | **● add** | ● (auto only → **also suggest**) | · |
| process_ai | · | ● | **● add** | **● add** | · |
| dismiss_todo | ● | · | **fix (broken)** | · | · |
| follow / unfollow | ● | · | **● add** | · | · |
| undo | **● add (post-action)** | **● add** | · | · | ● |

## 6. Known gaps & bugs (backlog)

**Done**
- ✅ **Scout invents tags** → prompt now lists the workspace's real tags and forbids inventing names (`Ai::EmailChatService#system_message`). Verified: Scout picks `security_flagged` (real) instead of `security`.
- ✅ **`Tools::AddTag` matched tags globally** → now scoped to the email's workspace.
- ✅ **Phase 1 (partial): the registry exists.** `EmailActions` (`app/services/email_actions.rb`) is the canonical registry — metadata (label, target, perm, destructive, surfaces) + execution wrapping `Tools::*`, with one permission gate. **Both Scout paths route through it**: `Tools::Executor` is now a thin delegate, and `EmailToolsController#create` dispatches its shared tools (add_tag/remove_tag/archive/trash/snooze/unsnooze/forward_email) through it. Failures now surface the specific reason instead of "Action failed." *(Still to migrate: `BulkController`, Cmd+K, Skim.)*
- ✅ **Two latent bugs fixed** (newly reachable once `add_tag` could actually succeed): `ActionController::Parameters` handling in the registry, and a bare `dom_id` in `EmailToolsController` that would 500 on a successful tag/untag render.
- ✅ **Action labels** are explicit ("Tag: invoice" / "Remove tag: invoice"), not bare values.
- ✅ **Cmd+K "Dismiss AI todo"** now PATCHes `…/dismiss_todo` instead of just reloading; removed the dead `save_compose_draft` route.
- Specs: `spec/services/email_actions_spec.rb`, `spec/requests/email_tools_spec.rb` (+ `scoping_spec` still green).

**Execution-layer mismatches**
- `Tools::Executor` lacks `draft_reply / send_reply / save_draft / send_draft / discard_draft`; `EmailToolsController` lacks `bulk_archive / bulk_tag / reclassify`. → unify under the registry.
- **`delete` vs `trash` semantics differ** — `BulkDelete` destroys DB rows; `Tools::Trash` only moves to the Trash folder. Pick one meaning per verb.
- **Skim reply re-implements** `EmailToolsController#send_reply`; **Skim undo** (`Emails::SkimRestore`) has no symmetric `Tools::BulkUnarchive`. → route through the registry; add a generic `undo`.
- `Tools::AddTag` looks up by name; the manual UI uses tag **id**. Registry should accept either, resolving names against the live list.

**Dead / broken code**
- **Bulk forward is dead**: `BulkController#dispatch_tool` has no `forward` case, no toolbar button emits it, and `Tools::BulkForward` is never called (`bulk_controller.rb`, `tools/bulk_forward.rb`).
- **`POST /email_messages/:id/save_compose_draft`** routes to `EmailComposeController#save_draft`, which doesn't exist (`config/routes.rb` ~L193). Dead route.
- **Cmd+K "Dismiss AI todo"** does `Turbo.visit(/email_messages/:id)` instead of `PATCH …/dismiss_todo` — it reloads the page and never dismisses (`command_palette_controller.js`).

**Coverage gaps**
- No single-message **mark read/unread** control; no Scout tool for it (read is auto-on-open only).
- **Move to folder** is bulk-only — no single-message button and no Scout tool.
- **reclassify / process_ai / scout_chat** are reachable from only one surface each.
- Cmd+K is missing most thread actions: snooze, trash, mark read/unread, delete, labels, follow, bulk tag/snooze, process_ai, scout_chat.
- Skim only does keep / archive / undo / reply — no snooze, trash, tag, move, mark-read, forward.

## 7. Phased plan

1. **Registry + execution unification.** Introduce `EmailActions`, wrap existing `Tools::*` in action definitions, and migrate `EmailToolsController`, `Tools::Executor`, and `BulkController` to dispatch through it (no behavior change). Reconcile `trash` vs `delete`.
2. **Ground Scout from the registry.** Generate Scout's tool list and the live-state injections (tags ✅, folders next) from the registry; keep the "no inventing" rule. Make `Tools::Executor` and `EmailToolsController` share the exact registry tool set.
3. **Generate the surfaces.** Drive the Cmd+K context commands and the bulk toolbar from `EmailActions.available_for(...)`. Add the missing single-message controls (mark read/unread, move-to-folder, archive/trash buttons).
4. **Fill Skim + Cmd+K parity.** Add snooze/trash/tag/move/mark-read to Skim and the missing palette commands; add a generic `undo`.
5. **Kill dead code, fix bugs.** Bulk forward, `save_compose_draft` route, palette dismiss-todo. Consistent labels (verb + object) and specific error messages everywhere.
6. **Tests.** One request/service spec per action asserting it behaves identically across surfaces and respects permissions.

## 8. Acceptance criteria

- Adding a new action requires editing **one** registry entry; it then appears (where permitted) in the email UI, Cmd+K, Scout, and Skim automatically.
- Scout never proposes an action that would fail for a "doesn't exist / no permission" reason — it's constrained to live state.
- Every action chip/button reads as a clear verb + object; every failure names the specific reason.
- `trash` and `delete` have one agreed meaning each, used identically everywhere.
