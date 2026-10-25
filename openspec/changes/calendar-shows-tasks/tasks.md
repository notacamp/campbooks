# Tasks

## 1. Tasks feed the calendar

- [ ] 1.1 `tasks` client: export `useTasks` from `~/modules/tasks` (public surface); no behaviour change
- [ ] 1.2 `schedule/due.ts`: derive the calendar's task rows from `useTasks` (open, mine): a Due row on `due_on` (today once past), a Planned row on `plan_on` for `plan = date` (today once past), one row per task per day with Due winning; drop the cache-only stub and `DUE_ACTION_KEY`; unit tests for the day rules (planned and due apart, same day, slipped plan, overdue, Later, no dates)
- [ ] 1.3 App shell: plug `useConversationSources` into `buildScheduleRoutes`, as for `buildTasksRoutes`; the sheet and the Agenda row show the email's subject and sender

## 2. The views and the sheet

- [ ] 2.1 Agenda: Planned rows ("Planned", "Planned <weekday>" once slipped) beside Due rows, ordered as design.md says
- [ ] 2.2 Week: the lane is labelled "Tasks"; Due pills dashed, Planned pills solid
- [ ] 2.3 Month: a day's tasks after its events, counted in "+N more"
- [ ] 2.4 Sheet: Due or Planned chip, planned day and due day, source, Done / Paid through the tasks module's actions (with Undo), Open task; no Hold time and no Later
- [ ] 2.5 Strings in en, pt-PT, es-ES and fr; copy fits at 375px in every language

## 3. Scout and stories

- [ ] 3.1 `crates/modules/schedule/handbook.md`: planned tasks show on the calendar too; ask the tasks for them
- [ ] 3.2 Calendar stories seed tasks through the mocked `tasks.list` (not the query cache): planned and due apart, same day, slipped, overdue; a play-less story for the recording

## 4. Before merge

- [ ] 4.1 Changelog entry file (`changelog/unreleased/added/`); `openspec validate calendar-shows-tasks --strict`; gates green in CI
- [ ] 4.2 Recording (375px and desktop), then archive this change
