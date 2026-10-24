# Polish: the task form, the calendar's views, Home's spacing, date and time pickers

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/347

## Why

Using Tasks, the Calendar and Home day to day showed five rough edges:

- Adding a task happens in a small form squeezed under the list's header. It
  should be central: in the right pane, in the shape of a task's page.
- The Calendar's Agenda / Week / Month switch is taller and louder than the
  title beside it.
- On Home, groups, the Scout noticed note and an opened item run into one
  another with no space between them.
- Scout noticed sets its long sentence in the display face, which is hard to
  read, and its five buttons wrap to two rows. There is no way to see the
  emails it counted.
- Dates and times use the browser's native inputs, which look different on
  every platform, can hold a half-typed year, and do not follow the member's
  language.

## What changes

- **Tasks (client, `tasks` module):** "New task" (N T) opens a new-task form
  in the right pane (`/w/{id}/tasks?new=1`), laid out like a task's detail: the
  title as the page's title, Do or Pay, a Due section with the due chips, then
  Add task and Cancel. Adding keeps the form open and empty for the next one.
  The inline quick add under the list's header goes away.
- **Calendar (`lib/ui` Calendar):** the view switch is compact, sized like the
  other segmented switches, with small, quiet key hints.
- **Home (`attention` module, `lib/ui` Item):** groups, Scout noticed and an
  opened item keep a section's room between them.
- **Scout noticed (`conversations` module, `lib/ui` Note and OverflowActions):**
  a title line ("Scout noticed", "1 of 5") over the sentence set in the body
  font; "See the N emails" (Shift+S) opens the search for the sender's mail in
  the window Scout counted; the button row keeps the verb, Something else… and
  Not now, and moves Don't suggest this and Ask Scout into More, so it never
  wraps.
- **Design system (`lib/ui`):** `DatePicker` (a field that opens a month grid,
  or the grid in place) and `TimePicker` (a time combobox) replace every native
  date and time input: the due chips' Pick a date, a task's Pick a day and
  Later, the event form, working hours and the sign-up codes' expiry.

Client only: no endpoint, event, job or tracked event changes.

## Non-goals

- Reading a date from the words of a task's title (still never).
- A date range picker, or times across midnight in one field.
- Changing what Scout noticed suggests, or the search grammar.
- A new layout for the event form beyond its date and time fields.
