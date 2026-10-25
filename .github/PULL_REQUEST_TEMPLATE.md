<!-- Terse, for a reviewer who knows the codebase. Title = Conventional Commit (it becomes the squash commit). -->

Refs notacamp/campbooks-cloud#<!-- issue --> · OpenSpec change: `openspec/changes/<!-- name -->`

## What changed
<!-- Bullets. What and why, not how. -->

## Verification
- [ ] Gates green locally (see AGENTS.md → Code quality)
- [ ] OpenAPI + client types regenerated (if the API changed)
- [ ] Changelog entry file in `changelog/unreleased/<kind>/` (feat/fix; `CHANGELOG.md` untouched)
- [ ] Tracked events: Coverage rows/events for every new operation, job, schedule and handler (docs/tracking.md)
- [ ] User-facing text in en, pt (pt-PT), es and fr, with full diacritics (`pnpm i18n:check`)
- [ ] UI verified at 375px and desktop (if UI) — recording below, demo data only

## Recording
<!-- REQUIRED when the PR changes UI (CI checks it): a video or GIF of the change working at 375px AND desktop.
     Record: `pnpm --dir clients record <scenario>`. Maintainers: `upload-media <file>` and paste its markdown.
     Contributors: drag the .webm/.mp4 into this box. Demo data only — never a real inbox, document or face. -->

## Follow-ups (out of scope)
