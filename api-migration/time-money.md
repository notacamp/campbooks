# Time & Money — `/api/app`

**Status:** ⬜ not started · **Priority:** P2 · **Depends on:** [`00-auth.md`](00-auth.md)

Two independent but architecturally similar surfaces. Both have rich server-side aggregation
services that the SPA simply needs to expose as JSON. Time is medium scope; Money is large
(statement upload, reconciliation workbench, loans).

---

## TIME

The merged agenda surface (`/time`): calendar events + deadlines (Reminders) + live asks
(Tasks) + Scout-proposed FocusBlocks in one chronological list, plus a Scout day note and
slot-finder for focus time. Undated asks live below the timeline.

### What the SPA must render

- Merged agenda for the requested day/week range, bucketed by date in the user's effective
  time zone (`User#effective_time_zone` — user zone → primary calendar zone → UTC)
- "No date yet" undated asks section (separate from the sorted timeline)
- Scout day note (`Time::DayNote`) — a pure-AI prose line for the day
- Focus block suggestions from `Time::FocusProposer` / `Time::SlotSuggester`
- Actions on asks: hold (stake a FocusBlock via `Time::FocusHolder`), schedule (set date),
  snooze, done, dismiss, hand off, take back
- Actions on focus blocks: keep (convert to real calendar event via `Time::FocusKeeper`),
  move, dismiss
- Actions on reminders: confirm (converts to calendar event), dismiss, snooze
- Undo for all mutations
- `PATCH account/time_zone` — one-time silent capture from the `local-greeting` Stimulus
  controller; the SPA equivalent fires once when `User#time_zone` is blank

### Web routes being replaced

| Web route | Controller#action | Notes |
|---|---|---|
| `GET /time` | `TimeController#index` | `?view=agenda\|week\|month`, `?date=` |
| `POST /focus_blocks/:id/keep` | `FocusBlocksController#keep` | Converts to calendar event (`Time::FocusKeeper`) |
| `PATCH /focus_blocks/:id/move` | `FocusBlocksController#move` | Reschedules a block |
| `DELETE /focus_blocks/:id` | `FocusBlocksController#dismiss` | Removes the suggestion |
| `POST /asks/:id/hold` | `AsksController#hold` | Stakes a FocusBlock in Scout's free slot (`Time::FocusHolder`) |
| `PATCH /asks/:id/schedule` | `AsksController#schedule` | Sets `due_on` |
| `POST /asks/:id/snooze` | `AsksController#snooze` | `tasks.snoozed_until` |
| `POST /asks/:id/done` | `AsksController#done` | Marks complete |
| `POST /asks/:id/dismiss` | `AsksController#dismiss` | Removes from view |
| `POST /asks/:id/hand_off` | `AsksController#hand_off` | Reassigns to teammate (`Asks::HandOff`) |
| `POST /asks/:id/take_back` | `AsksController#take_back` | Reclaims from teammate |
| `GET /reminders` | `RemindersController#index` | Pending deadlines list |
| `POST /reminders/:id/confirm` | `RemindersController#confirm` | Converts to calendar event |
| `DELETE /reminders/:id` | `RemindersController#dismiss` | Dismisses |
| `POST /reminders/:id/snooze` | `RemindersController#snooze` | Snoozes |
| `PATCH account/time_zone` | `Settings::AccountController#time_zone` | One-shot zone capture |

### `/api/v1` coverage today

🟡 **Partial.** Raw resources exist:
- `Api::V1::TasksController` — CRUD on tasks/asks; serializer: `Api::V1::TaskSerializer`
- `Api::V1::RemindersController` — list/show/CRUD; serializer: `Api::V1::ReminderSerializer`
- `Api::V1::CalendarEventsController` — list/show/CRUD; serializer: `Api::V1::CalendarEventSerializer`

**Not covered:** `Time::Agenda` (the merged multi-source aggregation), `Time::DayNote`,
`Time::SlotFinder` / `Time::FocusProposer` / `Time::SlotSuggester`, `FocusBlock`
create/keep/move/dismiss, the undated-asks section, ask state-machine actions (`hold`,
`schedule`, `snooze`, `hand_off`, `take_back`), and the `account/time_zone` capture.

### `/api/app` endpoints to build

| Method + path | Purpose | Reuses |
|---|---|---|
| `GET  /api/app/time` | Full agenda: `{ items, undated, day_note, suggestions }` for requested range | `Time::Agenda.for(user, from:, to:)`, `Time::Agenda.undated_for(user)`, `Time::DayNote`, `Time::FocusProposer`, `Calendars::PageData.for(user:, view:, date:, entitlements:)` |
| `PATCH /api/app/account/time_zone` | Capture user time zone | `Settings::AccountController#time_zone` logic, `User#time_zone=` |
| `POST  /api/app/asks/:id/hold` | Stake a focus block | `AsksController#hold` logic, `Time::FocusHolder` |
| `PATCH /api/app/asks/:id/schedule` | Set `due_on` | `AsksController#schedule` logic |
| `POST  /api/app/asks/:id/snooze` | Snooze the ask | `AsksController#snooze` logic |
| `POST  /api/app/asks/:id/done` | Mark done | `AsksController#done` logic |
| `POST  /api/app/asks/:id/dismiss` | Dismiss | `AsksController#dismiss` logic |
| `POST  /api/app/asks/:id/hand_off` | Reassign to teammate | `Asks::HandOff` |
| `POST  /api/app/asks/:id/take_back` | Reclaim from teammate | `AsksController#take_back` logic |
| `POST  /api/app/focus_blocks/:id/keep` | Convert FocusBlock → real event | `Time::FocusKeeper` |
| `PATCH /api/app/focus_blocks/:id/move` | Reschedule a FocusBlock | `FocusBlocksController#move` logic |
| `DELETE /api/app/focus_blocks/:id` | Dismiss a FocusBlock suggestion | `FocusBlocksController#dismiss` logic |
| `POST  /api/app/reminders/:id/confirm` | Convert reminder → calendar event | `RemindersController#confirm` logic |
| `DELETE /api/app/reminders/:id` | Dismiss reminder | `RemindersController#dismiss` logic |
| `POST  /api/app/reminders/:id/snooze` | Snooze reminder | `RemindersController#snooze` logic |

All ask/focus-block/reminder mutations return `{ item, undo_token }` sufficient for
optimistic update + undo. The `GET /api/app/time` response includes the full re-computed
`items` and `undated` arrays (same pattern as the `surface=money` contract in Money — see
below).

### Read models / serializers needed

- `Api::App::Time::AgendaSerializer` — wraps `Time::Agenda` output: `items` (array of
  `AgendaItemSerializer`), `undated` (array of ask cards), `day_note` (string), `suggestions`
  (array of slot descriptors from `Time::FocusProposer` / `Time::SlotSuggester`)
- `Api::App::Time::AgendaItemSerializer` — one `Time::AgendaItem` (a `Data.define` value
  object): `id`, `kind` (`:event`, `:ask`, `:reminder`, `:focus_block`), `title`,
  `starts_at`, `ends_at`, `all_day`, `source_id`, `source_type`, `color`, `status`, plus
  kind-specific extras (`ask_verb`, `instalment_label`, etc.)
- `Api::App::Time::FocusBlockSerializer` — `FocusBlock` model fields + proposed slot
- Reuse `Api::V1::TaskSerializer`, `Api::V1::ReminderSerializer`, `Api::V1::CalendarEventSerializer`
  as data sources; wrap or extend rather than duplicate

### Real-time

The `time` surface does not have a dedicated Turbo channel today. Ask state changes already
broadcast on `people_<user_id>` (People lanes) and the feed channel. For the SPA, either:
- Subscribe to the existing `people_<user_id>` JSON channel and re-query `/api/app/time`
  on relevant events, **or**
- Add a dedicated `time_<user_id>` channel that pushes agenda-item patches when asks/events
  change.

Recommendation: re-query on `people_<user_id>` events first (simpler), introduce a dedicated
channel if latency is unacceptable. Decision deferred to [`realtime.md`](realtime.md).

### Open questions

- Does the SPA week/month view need the full `Calendars::PageData` calendar-events breakdown,
  or is the flat `Time::Agenda` items array sufficient? (`TimeController#index` already shares
  `Calendars::PageData` with `CalendarController` — verify overlap with
  [`calendar.md`](calendar.md) to avoid duplicate endpoints.)
- `Time::DayNote` is AI-generated. Should `/api/app/time` block on it or return it async
  (poll / stream)? The web view renders it synchronously today.
- Slot suggestions from `Time::FocusProposer` / `Time::SlotSuggester` — are these pre-computed
  on load or lazily requested? Clarify before designing the response shape.

---

## MONEY

Evidence-driven obligations, bank-statement reconciliation workbench, and loan tracking. The
single builder `Money::Page.for(workspace, user, today:, statement_id:)` drives the entire
surface; the SPA equivalent is that every money mutation returns the refreshed Money read model
(the `surface=money` contract from `BankTransactionsController#render_workbench_streams`).

### What the SPA must render

- Obligations (ledger rows): due/overdue invoices + amounts with evidence-driven statuses
  (`Money::Ledger` / `Money::Evidence`; nothing shown as late until `GRACE_DAYS = 7` after a
  ready reconciliation covers the anchor date)
- "Needs you" items (`Money::Page#needs_you`) — up to 8 high-priority actions
- Active loans list (`Money::Page#loans`), loan-suggestion cards (`Money::Page#loan_suggestions`)
- Obligation actions: chase, settle, unsettle (id pattern `doc:<uuid>`)
- Statement upload, list, and inline reconciliation workbench: match/confirm/reject/exclude/reset/
  manual-match per bank transaction line
- Reconciliation actions: confirm-all suggestions, export CSV, retry parse, download original
- Loan CRUD: create, update, destroy, show, dismiss suggestion
- Money-surface contract: after any workbench action the full `Money::Page` re-renders

### Web routes being replaced

| Web route | Controller#action | Notes |
|---|---|---|
| `GET /money` | `MoneyController#index` | Main obligations view |
| `GET /money/statements` | `MoneyController#statements` | Statement list |
| `GET /money/statements/:id` | `MoneyController#statement` | Single statement (opens workbench) |
| `GET /money/export` | `MoneyController#export` | CSV export of ledger |
| `POST /money/reconcile_statements` | `MoneyController#reconcile_statements` | Trigger match job |
| `POST /money/obligations/:id/chase` | `MoneyController#chase` | Draft a chase email |
| `PATCH /money/obligations/:id/settle` | `MoneyController#settle` | Mark as settled |
| `PATCH /money/obligations/:id/unsettle` | `MoneyController#unsettle` | Revert settled |
| `POST /money/obligations/:id/confirm_line` | `MoneyController#confirm_line` | Confirm a workbench line from Money surface |
| `POST /money/obligations/:id/set_aside_line` | `MoneyController#set_aside_line` | Set aside a line |
| `POST /money/obligations/:id/reset_line` | `MoneyController#reset_line` | Reset a line |
| `GET /money/loans` | `LoansController` (index implied) | Loan list |
| `POST /money/loans` | `LoansController#create` | Create loan |
| `GET /money/loans/:id` | `LoansController#show` | Loan detail |
| `PATCH /money/loans/:id` | `LoansController#update` | Update loan |
| `DELETE /money/loans/:id` | `LoansController#destroy` | Delete loan |
| `POST /money/loans/:id/dismiss` | `LoansController#dismiss` | Dismiss loan suggestion |
| `GET /reconciliations/new` | `ReconciliationsController#new` | Upload form |
| `POST /reconciliations` | `ReconciliationsController#create` | Upload + parse statement |
| `GET /reconciliations/:id` | `ReconciliationsController#show` | Workbench |
| `DELETE /reconciliations/:id` | `ReconciliationsController#destroy` | Remove statement |
| `POST /reconciliations/:id/confirm_all_suggestions` | `ReconciliationsController#confirm_all_suggestions` | Bulk confirm |
| `GET /reconciliations/:id/export` | `ReconciliationsController#export` | CSV export |
| `POST /reconciliations/:id/retry_parse` | `ReconciliationsController#retry_parse` | Re-run CSV parser |
| `GET /reconciliations/:id/download` | `ReconciliationsController#download` | Download original file |
| `POST /reconciliations/:id/bank_transactions/:id/confirm` | `Reconciliations::BankTransactionsController#confirm` | `Reconciliations::LineActions#confirm!` |
| `POST /reconciliations/:id/bank_transactions/:id/reject` | `Reconciliations::BankTransactionsController#reject` | `Reconciliations::LineActions#reject!` |
| `POST /reconciliations/:id/bank_transactions/:id/exclude` | `Reconciliations::BankTransactionsController#exclude` | `Reconciliations::LineActions#exclude!` |
| `POST /reconciliations/:id/bank_transactions/:id/reset` | `Reconciliations::BankTransactionsController#reset` | `Reconciliations::LineActions#reset!` |
| `POST /reconciliations/:id/bank_transactions/:id/manual_match` | `Reconciliations::BankTransactionsController#manual_match` | `Reconciliations::LineActions#manual_match!` |
| `POST /reconciliations/:id/bank_transactions/:id/request_invoice` | `Reconciliations::BankTransactionsController#request_invoice` | Draft invoice-request email |
| `POST /reconciliations/:id/bank_transactions/:id/upload_and_link` | `Reconciliations::BankTransactionsController#upload_and_link` | Upload + attach document to line |
| `GET /reconciliations/:id/bank_transactions/:id/resolve_panel` | `Reconciliations::BankTransactionsController#resolve_panel` | Panel for manual-match drawer |

### `/api/v1` coverage today

❌ **None.** `/api/v1` has no coverage for Money, reconciliation, statements, bank
transactions, loans, or evidence. Everything is net-new.

### `/api/app` endpoints to build

| Method + path | Purpose | Reuses |
|---|---|---|
| `GET  /api/app/money` | Full money read model: obligations, needs-you, loans, loan suggestions | `Money::Page.for(workspace, user, today:)` |
| `GET  /api/app/money/export` | CSV ledger export | `MoneyController#export` logic |
| `POST /api/app/money/reconcile_statements` | Trigger `Reconciliations::MatchJob` for workspace | `MoneyController#reconcile_statements` logic |
| `POST /api/app/money/obligations/:id/chase` | Draft chase email → `{ draft_id }` | `MoneyController#chase` logic |
| `PATCH /api/app/money/obligations/:id/settle` | Settle obligation → refreshed money read | `MoneyController#settle` logic |
| `PATCH /api/app/money/obligations/:id/unsettle` | Revert → refreshed money read | `MoneyController#unsettle` logic |
| `POST /api/app/money/obligations/:id/confirm_line` | Confirm workbench line from Money surface | `MoneyController#confirm_line` logic, `Reconciliations::LineActions#confirm!` |
| `POST /api/app/money/obligations/:id/set_aside_line` | Set aside line | `MoneyController#set_aside_line` logic |
| `POST /api/app/money/obligations/:id/reset_line` | Reset line | `MoneyController#reset_line` logic, `Reconciliations::LineActions#reset!` |
| `GET  /api/app/reconciliations` | List statements (tabs in the workbench) | `Money::Page#statement_counts` |
| `POST /api/app/reconciliations` | Upload + parse bank statement | `ReconciliationsController#create` logic, `Reconciliations::CsvParser` (verify) |
| `GET  /api/app/reconciliations/:id` | Workbench: transactions + match suggestions | `ReconciliationsController#show` logic, `Money::Page.for(..., statement_id:)` |
| `DELETE /api/app/reconciliations/:id` | Remove statement | `ReconciliationsController#destroy` logic |
| `POST /api/app/reconciliations/:id/confirm_all_suggestions` | Bulk-confirm all suggestions | `ReconciliationsController#confirm_all_suggestions` logic |
| `GET  /api/app/reconciliations/:id/export` | CSV export of reconciliation | `ReconciliationsController#export` logic |
| `POST /api/app/reconciliations/:id/retry_parse` | Re-run CSV parser | `ReconciliationsController#retry_parse` logic |
| `GET  /api/app/reconciliations/:id/download` | Download original statement file | `ReconciliationsController#download` logic |
| `POST /api/app/reconciliations/:rec_id/bank_transactions/:id/confirm` | Confirm match → refreshed money read | `Reconciliations::LineActions#confirm!` |
| `POST /api/app/reconciliations/:rec_id/bank_transactions/:id/reject` | Reject suggestion → refreshed money read | `Reconciliations::LineActions#reject!` |
| `POST /api/app/reconciliations/:rec_id/bank_transactions/:id/exclude` | Exclude line → refreshed money read | `Reconciliations::LineActions#exclude!` |
| `POST /api/app/reconciliations/:rec_id/bank_transactions/:id/reset` | Reset line → refreshed money read | `Reconciliations::LineActions#reset!` |
| `POST /api/app/reconciliations/:rec_id/bank_transactions/:id/manual_match` | Manual-match to document → refreshed money read | `Reconciliations::LineActions#manual_match!` |
| `POST /api/app/reconciliations/:rec_id/bank_transactions/:id/request_invoice` | Draft invoice-request email → `{ draft_id }` | `BankTransactionsController#request_invoice` logic |
| `POST /api/app/reconciliations/:rec_id/bank_transactions/:id/upload_and_link` | Upload file + attach to line | `BankTransactionsController#upload_and_link` logic |
| `GET  /api/app/reconciliations/:rec_id/bank_transactions/:id/resolve_panel` | Manual-match search panel data | `BankTransactionsController#resolve_panel` logic |
| `GET  /api/app/money/loans` | Active loans + instalments | `Money::Page#loans`, `Loan` + `LoanInstalment` |
| `POST /api/app/money/loans` | Create loan | `LoansController#create` logic, `Loans::Backfill` |
| `GET  /api/app/money/loans/:id` | Loan detail + instalment schedule | `Loans::Status.refresh!`, `Loans::Schedule` |
| `PATCH /api/app/money/loans/:id` | Update loan | `LoansController#update` logic |
| `DELETE /api/app/money/loans/:id` | Delete loan | `LoansController#destroy` logic |
| `POST /api/app/money/loans/:id/dismiss` | Dismiss loan suggestion | `LoansController#dismiss` logic |

**`surface=money` contract for the SPA:** every bank-transaction and obligation mutation
response includes a top-level `money` key containing the refreshed `Money::Page` read model
(same data as `GET /api/app/money`). This lets the client patch its TanStack Query cache in
one response rather than issuing a follow-up read.

### Read models / serializers needed

- `Api::App::Money::PageSerializer` — the full `Money::Page` read: `evidence` (status,
  grace_days, covered?), `obligations` (ledger rows via `Money::Ledger`), `needs_you`
  (array of `Money::NeedsYouItem`), `loans`, `loan_suggestions`, `statement_counts`
- `Api::App::Money::ObligationSerializer` — a `Money::Ledger` row: document id/title/type,
  anchor_date, amount, evidence-driven `status` (`:unconfirmed`, `:missing`, `:confirmed`,
  `:settled`), `late?`, `money_data` (amounts for late-invoice chips)
- `Api::App::Money::LoanSerializer` — `Loan` + derived `Loans::Status` (rate, `missed` /
  `expected` / `unverified` instalments, `previous_amount_cents`)
- `Api::App::Money::LoanInstalmentSerializer` — `LoanInstalment` row
- `Api::App::Money::ReconciliationSerializer` — `Reconciliation` header + `statement_counts`
- `Api::App::Money::BankTransactionSerializer` — `BankTransaction` with match suggestions,
  current status, `RESOLVED_STATUSES` membership, loan-explained label
- `Api::App::Money::NeedsYouItemSerializer` — (verify `Money::NeedsYouItem` shape) priority
  action cards (newest statement + loan alerts)

### Real-time

The Money surface has no live Turbo channel today. Reconciliation is a deliberate, form-driven
workflow; real-time push is low priority. For the SPA:
- No dedicated channel required at launch; the client refetches `GET /api/app/money` after
  mutations (the response already carries the refreshed read model).
- If background jobs (e.g. `Reconciliations::MatchJob`, `Loans::Status.refresh!`) change
  the read model, a future `money_<workspace_id>` channel can push patches. Defer to
  [`realtime.md`](realtime.md).

### Open questions

- `upload_and_link` uploads a file to attach to a bank transaction line. What storage path does
  this use — Active Storage, and which model does the attachment belong to? Verify before
  designing the multipart endpoint.
- `request_invoice` drafts an email; the response should return a `draft_id` so the SPA can
  open the compose view. Confirm the draft shape aligns with [`compose-email.md`](compose-email.md).
- `Reconciliations::CsvParser` — verify the exact class name for the parse step in
  `ReconciliationsController#create` (file lists `csv_parser.rb` under
  `app/services/reconciliations/`; the full class name is `Reconciliations::CsvParser` — verify).
- `Money::NeedsYouItem` — verify the exact class name and fields (file listed as
  `app/services/money/needs_you_item.rb`).
- Loan `dismiss` action: does it set a workspace setting key (`workspace.settings["dismissed_loan_suggestions"]`)
  directly in the controller or via a service? Verify before implementing so the API path is
  consistent.
- Chase email: today `MoneyController#chase` renders a Turbo stream with a compose drawer.
  For the API, returning `{ draft_id }` and letting the SPA open compose is cleaner — confirm
  this aligns with the compose-email surface design.
