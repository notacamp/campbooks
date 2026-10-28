# Tasks

## 1. Settings and pickers

- [x] 1.1 Settings → Calendar: Mine / Following / Off groups with their descriptions; a segmented control per provider calendar (the Campbooks calendar shows Mine, no control); "Shared with you" when `owned === false`; Undo on a role change
- [x] 1.2 Hold time, "Add to my calendar" and "Also add to" offer only writable Mine calendars; an "Also add to" target that is no longer Mine shows as unset
- [x] 1.3 Strings in en, pt-PT, es-ES and fr; `pnpm test:fit` at 375px and 1280px
- [x] 1.4 Stories: Settings → Calendar with Mine, Following (shared) and Off calendars; a play-less story for the recording

## 2. Before merge

- [x] 2.1 Changelog entry file (`changelog/unreleased/added/399-calendar-roles-settings.md`)
- [ ] 2.2 `openspec validate calendar-roles-settings --strict`; CI green
- [ ] 2.3 Recording at 375px and on desktop, then archive this change
