# Tasks

## 1. Connectors read the list again

- [ ] 1.1 Migration: `connectors.streams.calendar_list_due_at`, spread over 24 hours for existing calendar streams
- [ ] 1.2 `contracts/connectors.calendar_list_read.v1.schema.json`; README and domain map; event catalogue (`cargo xtask events`)
- [ ] 1.3 `CalendarListRead` event; calendar runs read the list when due, publish it, remember new calendars off; an error only moves the marker
- [ ] 1.4 Tests: cadence, the published list, no read without the capability, provider error keeps event sync healthy

## 2. Schedule applies it

- [ ] 2.1 Extract the grant's rule into a shared function; `grant_calendars` uses it
- [ ] 2.2 `schedule.apply_calendar_list` ← `connectors.calendar_list_read`
- [ ] 2.3 Tests: a shared calendar appears off as following, an owned one off as mine, existing calendars unchanged (incl. a chosen role), `owned` filled
- [ ] 2.4 Handbook line

## 3. Before merge

- [ ] 3.1 Changelog entry file `changelog/unreleased/added/405-calendar-list-reread.md`
- [ ] 3.2 `openspec validate calendar-list-reread --strict`; CI green
- [ ] 3.3 Terminal recording; archive this change
