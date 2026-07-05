# Unified Action Catalog

> The complete inventory of every user action across **email, documents, calendar, and settings**, tagged against the model in [ADR 0002](adr/0002-unified-action-registry.md) and `CONTEXT.md`. This is the **build checklist** for the global Action registry. Derived from four inventory sweeps (June 2026); `file:line` references live in the sweep notes, not repeated here.
>
> Scope: this catalogs *resource operations and configuration*. Workflow **triggers** and **control-flow**, and the integration **effects** (`http_request`/`slack`/`discord`/`custom_action`), are deliberately **out** — they're workflow-only Steps, not Actions (see ADR 0002).

## Legend

- **kind** — `mut` mutation · `qry` query (read-only) · `nav` navigation
- **target** — the resource type, or `workspace` / `user` for Global Actions
- **card** — `1` single · `1+` one-or-many (collection from multi-select **or** a Target Projection)
- **level** — required Permission Level: `R` read · `W` write/send · `M` manage · `O` owner · `Ad` workspace admin (`User#admin?`). *(See refined ladder below.)*
- **auto** — Scout Autonomy **ceiling**: `F` forbidden (Cmd+K / human only) · `S` suggest (AI proposes, human approves) · `A` auto-capable (may run unsupervised if the workspace enables it). The *actual* level is set per-Group by workspace policy, never exceeding this ceiling.
- **dstr** — destructive (irreversible / data loss) in our sense
- **today** — current reach: `✓reg` in `EmailActions` · surfaces it's wired to · `—` controller-only

## Refined Permission Level ladder

The calendar + settings sweeps confirmed a **4th tier above `manage`**: both `EmailAccountUser` and `CalendarAccountUser` carry an `owner` flag, and owner-only actions exist (disconnect an account, manage its sharing). So the ladder is:

```
R read  →  W write/send  →  M manage  →  O owner          (per-resource flags)
                                          +  Ad workspace admin (User#admin?, Global Actions)
```

`O` and `Ad` are distinct: `O` is "creator of this shared account"; `Ad` is "workspace administrator." A Global Action needing elevation checks `Ad`; an owner-only resource action checks `O`.

---

## ⚠ Decisions the inventory surfaced (need a ruling before build)

1. **✅ RESOLVED — tighten to admin.** Workspace config is ungated today (only `Admin::*` checks `User#admin?`); the ruling is to require **`Ad`** for *all* workspace-scoped Global Actions — AI adapters, tags, document types, connections, integrations, inbox filter, workspace profile. This is a **deliberate behavior change**: non-admin members lose edit/delete access to shared config they can change today, so the settings slice must ship with that migration in mind (and a heads-up to existing workspaces). Tables below tag these `Ad`; `⚠member` marks where today's code still diverges and must be tightened. Membership/invitations (D4) keep their own approval gate — see there.
2. **✅ RESOLVED — collapse.** `block_sender` exists twice — as an email *sender* action (`EmailActions`) and as `set_sender_state` in inbox-filtering settings, same effect on the `Contact`. Collapse to **one** Action (`block_sender` et al.); the settings page becomes another surface for it.
3. **`reclassify` name collision** — `Tools::Reclassify` is **email-only**; document reclassify is a separate `Document#reclassify!` path. The registry needs two distinct actions (`reclassify_email` vs `reclassify_document`) or one polymorphic action keyed by target. (Recommend: two actions, same `classification` group.)
4. **Calendar `delete` is a soft tombstone** (`status: :cancelled`), not a destroy → tagged `dstr: no`. Only **document `merge`**, **doctype `destroy`**, and the hard account/tag/connection deletes are truly `dstr`.

---

## Target Projections (the fan-out graph)

| from → to | relation | enables |
|---|---|---|
| `email message`/`thread` → `document` | `document_email_messages` | "approve / push every attachment" |
| `thread` → `message` | thread has many messages | per-message actions on a thread |
| `message` → `sender (contact)` | `contact` / `Contacts::Identifier` | sender actions from a message |
| `email account` / `calendar account` → its `message`s / `event`s | account scope | account-wide bulk |

Projected sets are **permission-filtered**, run **per-item**, and report **skip-and-report** partial results.

---

## A. Email — `target: message | thread | sender`  ·  groups: `email_triage`, `email_send`, `email_sender`, `classification`

| Action | kind | target | card | level | auto | dstr | today |
|---|---|---|---|---|---|---|---|
| `add_tag` / `remove_tag` | mut | message | 1+ | R | A | no | ✓reg · single/bulk/palette/scout/wf (external tags sync to provider labels automatically) |
| `archive` | mut | thread | 1+ | R | A | no | ✓reg · single/bulk/palette/scout/skim/wf |
| `trash` | mut | thread | 1+ | R | S | no¹ | ✓reg · single/bulk/palette/scout/wf |
| `delete` (permanent) | mut | thread | 1+ | M | S | **yes** | — bulk only (`BulkDelete`) |
| `snooze` / `unsnooze` | mut | thread | 1+ | R | A | no | ✓reg · single/bulk/palette/scout/board |
| `mark_read` / `mark_unread` | mut | thread | 1+ | R | A | no | — bulk only |
| `move_to_folder` | mut | thread | 1+ | R | S | no | — bulk only |
| `forward_email` | mut | thread | 1+ | **W** | S | no² | ✓reg · single/scout/wf |
| `reply` / `reply_all` / `send_reply` | mut | message | 1 | **W** | S | no² | — `EmailComposeController` |
| `draft_reply` (AI) | mut | thread | 1 | R | S | no | — `Tools::DraftReply` |
| `dismiss_todo` | mut | message | 1 | R | S | no | — single |
| `follow` / `unfollow` | mut | thread | 1 | R | S | no | — single |
| `reclassify_email` | mut | message | 1+ | R | **A** | no | — scout_auto only |
| `process_ai` (re-analyze) | mut | message | 1+ | R | **A** | no | — bulk only |
| `star_sender` / `unstar_sender` | mut | sender | 1+ | R | S | no | ✓reg · single/bulk/palette/scout/skim |
| `block_sender` | mut | sender | 1+ | R | S | partial³ | ✓reg · single/bulk/palette/scout/skim |
| `unblock_sender` / `allow_sender` | mut | sender | 1+ | R | S | no | ✓reg · single/palette/skim |
| `query_emails` | qry | message | 1+ | R | A | no | — Scout read tool |

¹ recoverable (Trash folder). ² sends mail outward → `W`. ³ blocks the contact workspace-wide + archives their mail.

---

## B. Documents — `target: document`  ·  groups: `document_review`, `classification`, `document_export`

| Action | kind | target | card | level | auto | dstr | today |
|---|---|---|---|---|---|---|---|
| `upload_document` | mut | document (new) | 1+ | W | F | no | — UI only (human) |
| `view_file` / `download` | nav | document | 1 | R | A | no | — UI only |
| `rename_document` | mut | document | 1 | W | S | no | — UI only |
| `update_document_fields` | mut | document | 1 | W | S | no | — UI/skim (rich ~25-field arg schema) |
| `approve_document` | mut | document | 1+ | W | S | no | — UI/skim⁴ |
| `reject_document` / `dismiss` | mut | document | 1+ | W | S | no | — UI/skim (reversible) |
| `restore_document` (undo) | mut | document | 1 | W | S | no | — skim |
| `reclassify_document` | mut | document | 1+ | W | S | no | — skim (changes type + approves) |
| `reprocess_document` (re-run AI) | mut | document | 1+ | R | **A** | no | — UI/skim |
| `push_to_notion` | mut | document | 1+ | W | S | no | — UI (needs mapping) |
| `push_to_drive` (Google) | mut | document | 1+ | W | S | no | — UI (needs config) |
| `push_to_zoho_drive` | mut | document | 1+ | W | S | no | — UI (needs mapping) |
| `export_documents` (zip) | qry | document | many | R | S | no | — UI (filtered set) |
| `merge_documents` | mut | document | many (2+) | **M** | S | **yes** | — UI (hard-deletes non-kept + purges files) |
| `query_documents` | qry | document | 1+ | R | A | no | — Scout read tool ✓ |

⁴ skim approve defers finalize 7 s (undo window); list approve is immediate — same Action, surface-specific timing.

---

## C. Calendar — `target: calendar_event | calendar | calendar_account`  ·  groups: `calendar`, `calendar_settings`

| Action | kind | target | card | level | auto | dstr | today |
|---|---|---|---|---|---|---|---|
| `create_calendar_event` | mut | calendar_event (new) | 1 | W | S | no | ✓reg · single/palette/scout/wf (email-sourced); form-create is nav |
| `update_calendar_event` | mut | calendar_event | 1 | W | S | no | — UI (`recurrence_scope` arg) |
| `reschedule_calendar_event` | mut | calendar_event | 1 | W | S | no | — UI drag |
| `delete_calendar_event` | mut | calendar_event | 1 | W | S | no¹ | — UI (`recurrence_scope`) |
| `rsvp_calendar_event` | mut | calendar_event | 1 | W | S | no | — UI (`rsvp_status` arg) |
| `confirm_reminder` (→ event) | mut | calendar_event (new) | 1 | W | S | no | — Reminders page |
| `toggle_calendar_syncing` / `set_color` | mut | calendar | 1 | **M** | F | no | — settings |
| `connect_calendar_account` | mut | calendar_account (new) | 1 | — (→O) | F | no | — via email OAuth |
| `rename_calendar_account` | mut | calendar_account | 1 | **M** | F | no | — settings |
| `disconnect_calendar_account` | mut | calendar_account | 1 | **O** | F | soft | — settings (`active:false`) |
| `share_calendar_account` (grant/role/revoke) | mut | calendar_account_user | 1 | **O** | F | partial⁵ | — settings |

¹ soft tombstone. ⁵ revoke hard-deletes the share row.

---

## D. Settings — Global Actions  ·  `target: workspace | user | <account>`

All settings Actions have autonomy ceiling **`F`** (forbidden to Scout — Cmd+K / human only), per the rule that Scout never touches configuration. `⚠member` = today gates on membership only; design intends `Ad` (see Decision 1).

### D1. Workspace config — `target: workspace`, level **`Ad`** (tightened from member — a behavior change), group `settings_workspace`

| Action | dstr | notes |
|---|---|---|
| `update_workspace_profile` (name/type/context/location/tax) | no | also written by onboarding/setup wizards |
| `create_ai_adapter` / `update_ai_adapter` | no | provider + encrypted key |
| `delete_ai_adapter` | **yes** | blocked if configurations reference it |
| `assign_ai_configuration` (bind adapter→Purpose) | partial | blank adapter clears the config |
| `create_tag` / `update_tag` | no | group `settings_tags` |
| `delete_tag` | **yes** | `security_flagged` protected; drops all joins |
| `create_document_type` / `update_document_type` | no | group `settings_doctypes` |
| `delete_document_type` | **yes** | nullifies type on all docs + cascades Notion mapping |
| `update_inbox_filter_strategy` (blacklist/whitelist) | no | |
| `set_sender_state` (block/allow/unblock/unstar) | partial | **collapse into `block_sender` etc.** (Decision 2) |
| `create_connection` / `update_connection` | no | group `settings_integrations`; encrypted auth |
| `delete_connection` | **yes** | removes stored credentials |
| `connect_notion` / `disconnect_notion` | yes (disc.) | |
| `connect_google_drive` / `disconnect_google_drive` | yes (disc.) | |
| `connect_zoho_drive` / `disconnect_zoho_drive` | yes (disc.) | |
| `update_drive_config` / `update_drive_folder_mapping` / `delete_*_mapping` | yes (del.) | per-document-type auto-push |
| `update_notion_database_mapping` | — | ⚠ no write path today (model exists, controller gap) |

### D2. User config — `target: user`, level `self` (always own), group `settings_user`

| Action | dstr | notes |
|---|---|---|
| `change_language` | no | `User#locale` |
| `change_password` | no | requires current password |
| `delete_account` | **yes** | password + email confirm → `AccountDeletionJob` |
| `toggle_notification_preference` / `bulk_toggle` | no | per tag / doc-type, in-app + email |
| `create_signature` / `update_signature` / `set_default_signature` | no | own signatures |
| `delete_signature` | **yes** | |
| `trigger_manual_email_scan` | no | re-sync; `classification`-adjacent |

### D3. Account sharing — `target: email_account`, group `settings_sharing`

| Action | level | dstr | notes |
|---|---|---|---|
| `connect_email_account` (OAuth) | → O | no | acting user becomes owner |
| `rename_email_account` | **M** | no | |
| `disconnect_email_account` | **O** | **yes** | deactivates for all sharees |
| `share_email_account` (grant/role/revoke) | **O** | partial | revoke hard-deletes share row |

### D4. Members & invitations — `target: workspace`, group `settings_members`

| Action | level | dstr | notes |
|---|---|---|---|
| `invite_member` | M | no | cloud non-admin → pending approval |
| `cancel_invitation` | M | **yes** | |
| `resend_invitation` | M | no | admin resend auto-approves |
| `accept_invitation` (invitee) | self (token) | no | unauth/auth bearer |

*Membership keeps its existing gate — any member may invite, but in cloud mode a non-admin's invite needs admin approval (`admin_approved`). Separate from Decision 1; deliberately **not** blanket-admin'd.*

### D5. Admin — `target: workspace | app`, level **`Ad`** (enforced today ✓), group `settings_admin`

| Action | dstr | notes |
|---|---|---|
| `generate_beta_codes` | no | |
| `delete_beta_code` | **yes** | unredeemed only |
| `approve_signup_request` / `reject_signup_request` | reject=yes | |
| `approve_pending_invitation` / `reject_pending_invitation` | reject=yes | |
| `change_user_role` | partial | cannot target self |

---

## Build checklist (what the registry needs)

**New concepts / models**
- [ ] `DocumentUser` sharing model — defaults to **all workspace members** (no regression); narrows/elevates only. Gives `level(user, document)`.
- [ ] **Target Projection** graph (the 4 edges above) + permission-filtered, skip-and-report fan-out runner.
- [ ] Action definition schema: `kind`, `target`, `cardinality`, `level`, `group`, `autonomy ceiling`, typed `arguments`, `run`.
- [ ] **Workspace Autonomy Policy** setting (per-Group, capped by ceiling; `Ad`-gated). Itself a `settings_workspace` Action.
- [ ] `surfaces` loses `scout_*` (derived from `ceiling + policy`).

**Migration path (lowest-risk first)**
1. Scaffold the registry + `Actions.run` / `Actions.available_for`; **refactor `EmailActions` into the email slice in place** (specs exist — no behavior change).
2. Add `query`/`navigation` kinds; migrate Cmd+K + Scout tool-lists to generate from the registry.
3. Document slice: `DocumentUser` + the ~15 document Actions; wire approve/reclassify/reprocess into Scout + Cmd+K.
4. Calendar slice: the 11 calendar Actions; generalize the email→event entry.
5. Settings slice: Global Actions + the Autonomy Policy setting; resolve Decisions 1–2 first.
6. Generalize the workflow `email_action` Step → any resource Action (+ projection).

**Rulings:** Decisions 1 (✅ tighten to admin) and 2 (✅ collapse `block_sender`) resolved — settings slice unblocked. 3–4 tagged and ready. The admin-tightening ships as a deliberate behavior change (existing members lose edit/delete on shared workspace config).
