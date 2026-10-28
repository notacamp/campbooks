# Tasks

## 1. Client

- [x] 1.1 `useTaskHistory` (`tasks.get`), keyed by its operation so the change feed refetches it
- [x] 1.2 `history.ts`: one sentence per entry, by actor and kind, "A teammate" when the member is unknown, a plain fallback for kinds it does not know
- [x] 1.3 `TaskStory`: "Why it's here" and "History", on the desktop panel and the phone detail
- [x] 1.4 en, pt-PT, es-ES and fr strings

## 2. Tests and stories

- [x] 2.1 Vitest for the sentence builder and the fold
- [x] 2.2 Task detail tests: Scout-made, own, the history in order, the fold
- [x] 2.3 Stories: desktop and phone, each with a play-less twin

## 3. Close

- [x] 3.1 Changelog entry file
- [x] 3.2 Recording (owner session)
- [x] 3.3 `openspec archive task-story --yes`
