# Tasks

## 1. Proposing tool (`assistant`)

- [x] 1.1 Six task rows in the action table (operation, undo, `revision` copied; restoring rows for `tasks.plan` and `tasks.update`); catalogue test extended to every task row and to the two reads
- [x] 1.2 `propose_task_change`: narrow schema, strict argument checks (`day` a real date, only the detail the action takes)
- [x] 1.3 Checking: read the task and the members as the member; `not_visible`, `read_only`, `already_so`, `task_closed`, `until_out_of_range`, `day_out_of_range` (member's time zone), `not_a_teammate`; shared cap and dedupe; never invoke a mutating operation
- [x] 1.4 Stored with Scout's message: record, task facts from the record, detail (day, teammate id and name from the members list), request and undo; PATCH only for `tasks.update`
- [x] 1.5 The instruction names the tool; the line for the model after a settled task card

## 2. Server tests

- [x] 2.1 Each action stored with the right request and undo; restoring rows carry the previous values; each refusal; a task of a third member is `not_visible`; the cap and dedupe shared with the other tools; no change ever invoked

## 3. Client

- [x] 3.1 Runnable entries with exact path templates; PATCH allowed for `tasks.update` alone
- [x] 3.2 Cards for the six actions in four languages, the task and its due day as the fact line, the teammate named by the server
- [x] 3.3 Tests: runnable, the undo copying only `revision`, a task card confirmed and undone; Storybook screen story; recording scenario

- [x] 3.4 `taskRef` in `lib/scout-chat`, a `task` kind in the panel (about line, starters in four languages), `useTaskSubject`, "Ask Scout" with `.` on the task detail; tests; in the recording

## 4. Close

- [x] 4.1 OpenAPI and client types regenerated
- [x] 4.2 Changelog entry file
- [x] 4.3 Gates green
- [x] 4.4 UI recorded at 375px and desktop
- [x] 4.5 `openspec archive scout-proposes-task-changes --yes` before merge
