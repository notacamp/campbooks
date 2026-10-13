# Proposal

## Why

AGENTS.md makes four languages (`en`, `pt-PT`, `es-ES`, `fr`) a hard rule for
every string a user can read, and the server already sends some: the `title`
and `detail` of every problem+json error. Today they are English status
phrases and English sentences or bare codes. Emails and notifications come
next, and they are rendered in jobs, where no request says which language to
use. The server needs one way to pick a locale, one catalogue format with
plurals, localized errors, and a CI check that keeps catalogues at parity,
before any module adds more server text.

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/25.

## What Changes

- **Locale resolution.** `kernel::i18n::Locale` (`en`, `pt-PT`, `es-ES`,
  `fr`; always the full tag) is resolved per request from the user's saved
  locale (through a new kernel port, `LocalePreference`, implemented by
  `workspaces`), then `Accept-Language` (any `pt-*` → `pt-PT`, `es-*` →
  `es-ES`, `fr-*` → `fr`, `en-*` → `en`, others skipped), then `en`.
  Handlers take it as an extractor (`locale: Locale`); jobs carry it in
  their payload.
- **Message catalogues.** Each module (and the kernel) may hold
  `i18n/<locale>.json` in the format the client already uses (i18next JSON:
  nested keys, `{{name}}` placeholders, CLDR plural suffixes `_one`,
  `_many`, `_other` chosen by `count`). They are embedded at compile time
  (`kernel::catalogue!`) and rendered with `Catalogue::t(locale, key, args)`.
  Plural categories come from CLDR via `icu_plurals`.
- **Localized errors.** `kernel::Error`'s message-carrying variants
  (`BadRequest`, `Conflict`, `Unavailable`, `Denied`) carry a `Reason`: a
  stable code plus a catalogue message. Every problem+json response gains a
  stable `code` member; `title` (the status) and `detail` (the reason) are
  rendered in the request's locale, and `Content-Language` names it. `type`
  stays `about:blank`. **BREAKING** for API clients: the machine-readable
  reason moves from `detail` to `code` (e.g. `code: "last_owner"`), and
  `detail` becomes a sentence.
- **Parity check.** `cargo xtask i18n` (in CI) fails unless every server
  `i18n/` directory has exactly the four locale files, valid JSON with the
  same message keys, no empty values, the plural forms each locale needs
  and the same placeholders in every locale.
- **Data.** `workspaces.users.locale` becomes nullable with no default:
  `NULL` means "not chosen", so `Accept-Language` applies until the user
  picks a language.

## Capabilities

### New Capabilities

- `server-i18n`: locale resolution, server message catalogues and their
  parity check, localized problem+json errors, rendering for jobs.

### Modified Capabilities

None in the baseline. The in-flight `api-auth`, `workspaces` and `web-bff`
deltas now name reason codes as the problem's `code` instead of its
`detail`.

## Impact

- Crates: `campbooks-kernel` (new `i18n` module and kernel catalogue;
  `Error`/`Problem` reshaped; `localize` middleware installed by
  `operations::compose`; `AppState.locales`), `campbooks-server` (registers
  the port, `localize` layer for fallbacks and the web BFF, its own
  catalogue for the BFF's problems), `campbooks-ai` (catalogue for
  `ai_role_not_configured`), `campbooks-system`
  and `campbooks-workspaces` (catalogues, reasons; `workspaces` implements
  `LocalePreference`), `xtask` (`i18n` command).
- Dependencies: `icu_plurals` 2.3.0 and `icu_locale_core` 2.3.0 (Unicode-3.0,
  ICU4X; the locale core is already in the graph through `url`).
- Data: migration `workspaces__user_locale_unset` (owned by `workspaces`).
- OpenAPI: `Problem` gains `code`; client types regenerated.
- Self-hosters: one migration on startup; nothing to configure.

## Non-goals

- An endpoint to change the user's language (account settings, later); the
  UI keeps its own locale choice until then.
- Sending the UI's chosen locale as `Accept-Language` from the clients
  (client follow-up).
- Locale-aware number, date and currency formatting inside messages.
- Email or notification templates (none exist yet; the helper and the job
  pattern are documented).
- Localizing the identity provider's screens (issue #35).
- Gendered message variants; copy is written gender-neutral.
