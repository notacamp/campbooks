# Tasks

## 1. Kernel

- [x] 1.1 `i18n::Locale` (tags, serde, OpenAPI enum, `from_tag`, `Accept-Language` negotiation) with unit tests
- [x] 1.2 `Catalogue`, `catalogue!`, `t`, `Reason`, plural selection with `icu_plurals`, fallbacks; kernel catalogue in four locales; unit tests
- [x] 1.3 `check_catalogue` parity rules with unit tests
- [x] 1.4 `LocalePreference` port, `AppState.locales`, `Locale` extractor, lazy per-request slot, `localize` middleware; installed by `operations::compose` around idempotency
- [x] 1.5 `Error` variants carry `Reason`; `Problem.code`; English rendering by default; extractor rejections, JWKS and idempotency problems use kernel reasons; existing tests updated

## 2. Modules and server

- [x] 2.1 `system`: catalogue, health reasons
- [x] 2.2 `workspaces`: catalogue with every error code, reasons, `LocalePreference` implementation, migration `user_locale_unset`, `.sqlx`; tests (saved locale wins, unset falls back)
- [x] 2.3 Server: registers the port, outermost `localize` for fallbacks; tests (Accept-Language 404, 401 in French)

## 3. Parity gate

- [x] 3.1 `cargo xtask i18n`; CI step

## 4. Wrap-up

- [x] 4.1 AGENTS.md, `docs/architecture.md`, CHANGELOG; in-flight `api-auth`/`workspaces` scenarios name `code`
- [x] 4.2 OpenAPI regenerated, client types regenerated, `.sqlx` regenerated, gates green (no UI change)
