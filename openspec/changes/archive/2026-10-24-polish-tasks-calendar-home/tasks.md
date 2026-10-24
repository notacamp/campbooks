# Tasks

## 1. Design system

- [x] 1.1 `DatePicker` (`lib/ui/components/DatePicker`): field and inline modes, month grid with the ARIA grid keys, min and max, Today and Clear; pure day helpers in `dates.ts`; tests and stories
- [x] 1.2 `TimePicker` (`lib/ui/components/TimePicker`): ARIA 1.2 combobox over the day's slots, typed-time grammar, `min`; pure helpers in `times.ts`; tests and stories
- [x] 1.3 `Note` gains `title`, `meta` and `after`; `OverflowActions` gains labelled `buttons` and an always-folded `more`; `KbdHint` gains `size="sm"`; `TextField` gains `size="title"`; `Popover` gains `bottom-start` and `data-autofocus`; the `arrow-right` icon; stories and tests
- [x] 1.4 `ui` strings for the pickers in en, pt-PT, es-ES, fr

## 2. Tasks

- [x] 2.1 `NewTaskForm` in the right pane, opened by `?new=1` (header button, empty state, N T); QuickAdd and DateCommitField removed
- [x] 2.2 The due chips' Pick a date and a task's Pick a day and Later use the inline month grid; the detail's title uses the title field
- [x] 2.3 Tests, Tasks screen stories and the tasks recording scenario updated

## 3. Calendar, Home and Scout noticed

- [x] 3.1 Compact view switch with small key hints
- [x] 3.2 Home and Tasks lists space their groups; an opened item keeps its distance
- [x] 3.3 Scout noticed: title line, body font, See the N emails (Shift+S), More for Don't suggest this and Ask Scout
- [x] 3.4 The event form, working hours and sign-up codes use the pickers

## 4. Close

- [x] 4.1 Changelog entry files
- [x] 4.2 Gates green in CI
- [x] 4.3 Recording at 375px and desktop (`pnpm --dir clients record ui-polish`, from Storybook)
- [x] 4.4 `openspec archive polish-tasks-calendar-home --yes` before merge
