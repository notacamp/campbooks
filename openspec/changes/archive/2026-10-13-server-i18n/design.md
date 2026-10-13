# Design

## Context

The client localizes with i18next: one JSON file per locale per namespace
(`src/modules/<m>/i18n/<locale>.json`), `{{name}}` placeholders, plural
suffixes from `Intl.PluralRules`, parity checked by `pnpm i18n:check`. The
server's only user-facing text is problem+json: `title` is the English status
phrase and `detail` is an English sentence or a bare reason code
(`last_owner`, `invalid_token`). `workspaces.users` already has a `locale`
column (`en`/`pt-PT`/`es-ES`/`fr`), defaulting to `en`, that nothing sets.

## Data ownership, events, scopes

- **`workspaces`** owns the saved locale (`workspaces.users.locale`).
  Migration `<ts>_workspaces__user_locale_unset.sql` drops its default and
  `NOT NULL` and clears the stored defaults: no endpoint could ever set it,
  so every `en` in it is the default, not a choice. `NULL` = not chosen.
- **Kernel** owns nothing new in the database. It declares the
  `LocalePreference` port (like `Membership`); `workspaces` implements it
  with one primary-key read by IdP subject; the server registers it.
- **Events:** none published or consumed.
- **Endpoints:** none added, so no new scope. Every existing endpoint's error
  responses change shape (`code` added, `detail` localized).
- **AI:** none.

## Locale

`kernel::i18n::Locale` is a closed enum, serialized as its BCP 47 tag
(`en`, `pt-PT`, `es-ES`, `fr`) and documented in OpenAPI as that enum.
`Locale::from_tag` maps a language tag by primary subtag (`pt-BR` → `pt-PT`,
`es-MX` → `es-ES`: the closest translation beats English, as on the client).

Resolution, first match wins:

1. The caller's saved locale: the principal (attached in-process, or from a
   valid bearer token) is a user and `LocalePreference::preferred` returns a
   locale. Clients (`client_credentials`) have none. A port error is logged
   and skipped.
2. `Accept-Language`: entries sorted by `q` (stable), `q=0` and `*` skipped,
   first entry `from_tag` accepts.
3. `en`.

Resolution is **lazy**: it runs only when a handler asks for it (the
`Locale` extractor, infallible) or when a response carries a problem to
localize. Successful responses cost nothing. A per-request slot (request
extension) shares the result between the extractor and the middleware, so
the token is verified and the user read at most once more per request.

**Jobs** have no request: a job that renders text carries the recipient's
`Locale` in its payload, resolved when it is enqueued (the requester's own
`Locale`, or the recipient's saved locale read by the module that owns the
recipient). Payloads hold ids and a locale tag, never rendered text.

## Catalogues

- Files: `crates/kernel/i18n/<locale>.json` (namespace `kernel`) and
  `crates/modules/<m>/i18n/<locale>.json` (namespace `<m>`), one per locale.
  A module adds its directory when it first needs server text.
- Format: i18next JSON v4, as on the client. Nested objects of strings; a
  message id is the dotted path (`errors.last_owner`). Placeholders are
  `{{name}}`. A message with forms has keys `<id>_one`, `<id>_other` and,
  where the locale has it, `<id>_many` (`_zero`, `_two`, `_few` likewise);
  the `count` argument selects the CLDR cardinal category, falling back to
  `_other`.
- Embedding: `pub static MESSAGES: Catalogue = campbooks_kernel::catalogue!("<m>");`
  `include_str!`s the four files of the calling crate; they are parsed on
  first use. Missing in the locale → the `en` text → the key itself (and a
  warning): rendering never fails or panics.
- API: `MESSAGES.t(locale, "emails.invite.subject", &[("workspace", name.into())])`
  renders a message; `MESSAGES.reason("last_owner")` builds an error reason
  (code `last_owner`, message `errors.last_owner`), `.arg("max", 100)` adds
  an argument. Arguments are text or integers; values are inserted as is
  (callers escape for HTML when templates arrive).
- Plurals: `icu_plurals` (ICU4X, CLDR data compiled in), the same rules as
  the browser's `Intl.PluralRules`: `pt-PT` differs from `pt` (0 is
  `other`), and `fr`/`es-ES`/`pt-PT` have `many` for millions.

## Localized errors

- `Error::{BadRequest, Conflict, Unavailable, Denied}` carry a `Reason`
  instead of a `String` / `&'static str`. Every variant has a stable code:
  `not_found`, `forbidden`, `internal_error`, `insufficient_scope`, the
  `AuthFailure` codes, or the reason's code.
- `Problem` gains `code` (always set by the kernel). `type` stays
  `about:blank`, which RFC 9457 lets carry a title localized to the
  client's preference. `title` is the kernel catalogue's `status.<code>`
  (English reason phrase for statuses it does not list); `detail` is the
  reason's message (or the kernel's `errors.<code>`).
- `Error::into_response` renders English (so module tests and any response
  that bypasses the middleware are still complete), sets
  `Content-Language: en` and attaches the unrendered problem as a response
  extension. The kernel's `i18n::localize` middleware takes that extension
  off the response, resolves the locale lazily, re-renders the body in it
  and sets `Content-Language` (plus `Vary: Accept-Language`). Responses
  without the extension pass through untouched.
- Placement: `operations::compose` installs `localize` on both sides of the
  idempotency layer (inner: handler errors are localized before an
  idempotent response is stored, so a replay answers in the first request's
  language; outer: the idempotency layer's own problems), and the server
  adds it to its whole stack (inside compression) for fallback 404s and the
  web BFF's problems, whose reasons live in the server crate's catalogue. In-process `invoke` goes through the
  composed routes, so its problems are localized for the invoking principal.
- Extractor rejections become codes: `missing_json_content_type`,
  `invalid_json`, `invalid_body`, `invalid_query`; the parser's own English
  explanation is passed as the `{{reason}}` argument (developer-facing: a
  correct client never triggers them).

## Parity check

`kernel::i18n::check_catalogue(files)` implements the rules once; `cargo
xtask i18n` walks every `i18n/` directory under `crates/` and runs it (CI
step "Translations at parity (server)"); each catalogue's unit test runs it
on the embedded files too. Rules: exactly the four `<locale>.json` files,
valid JSON of nested strings, the same message ids as `en` (plural forms
grouped by id), `_one` and `_other` for every form message, no form a
locale's CLDR rules do not have, no empty value, and the same placeholders
as `en` (for forms, the union over forms without `count`).

## Alternatives considered

**Catalogue format**

- **Fluent (`fluent-bundle`).** Best expressiveness (selectors on any
  argument, gender, terms) and Mozilla-grade plural handling. But it is a
  second format next to the client's i18next JSON, so translators edit two
  syntaxes and the parity tooling needs an FTL parser; about eight new crates
  (`fluent-bundle`, `fluent-syntax`, `fluent-langneg`, `intl-memoizer`,
  `intl_pluralrules`, `unic-langid`, …); unifying would mean moving the
  client off i18next. Its extra power (gender selectors) is what our copy
  rule avoids (gender-neutral phrasing). Rejected for now; it remains the
  upgrade path if messages ever need selectors.
- **ICU MessageFormat.** One syntax for both sides is attractive, but ICU4X
  has no stable MessageFormat formatter yet and the Rust crates that exist
  are small and young; the client would need `i18next-icu`. Rejected.
- **gettext (`.po`).** Mature translator tooling, but `gettext-rs` links the
  C `libintl` (cross-compiling, musl, `cargo deny`), English source strings
  as keys make codes unstable, and nothing on the client reads `.po`.
  Rejected.
- **Plain key → string JSON, no plurals.** Leanest, shares the client file
  shape, but emails and notifications need counts ("3 new invitations").
  Rejected as too little.
- **Chosen: i18next JSON v4 on both sides**, rendered by ~300 lines in the
  kernel plus `icu_plurals`. One format and one mental model for
  translators; CLDR-exact plurals; no new runtime service.

**Plural rules:** hand-coded integer rules for four locales would add no
crate, but they must track CLDR (the `many` category for millions arrived in
CLDR 42; `pt-PT` differs from `pt`) and drift from the browser. ICU4X's core
crates are already in the graph through `url`, so `icu_plurals` adds its
data crate and three small ones. Chosen: `icu_plurals`.

**Where errors are localized**

- *Each handler takes `Locale` and renders its own errors.* Explicit, but
  every handler and every extractor rejection would need it, and forgetting
  it is silent. Rejected.
- *Middleware resolves the locale eagerly into a task-local that
  `Error::into_response` reads.* Simple to use, but every request (including
  successful ones) would pay a token verification and a database read.
  Rejected.
- *Chosen: errors carry codes and messages, rendered by one lazy
  middleware.* Handlers stay unchanged apart from naming reasons; the cost is
  paid only by error responses and by handlers that ask for `Locale`.

**Saved locale:** reading the OIDC `locale` claim instead of a port would
avoid a database read, but the identity provider's language is not the
user's choice in Campbooks, and not every IdP sends it. A port mirrors
`Membership` and keeps the kernel free of modules.

**Compared with the Rails app on `main`:** it resolved the locale as
`params` → user's saved locale → `Accept-Language` → default, kept YAML
catalogues per domain, and checked parity with `i18n-tasks`. We keep the
resolution order (it is the standard one and wins on its merits, minus the
query parameter, which an API does not need) and the idea of a parity gate;
we do not copy the YAML format, because sharing the client's format matters
more here.

## Risks / trade-offs

- **Breaking change for API clients**: the reason code moves from `detail`
  to `code`. Nothing in the client reads problem bodies yet.
- Other branches that build `Error::BadRequest(String)` / `Conflict(String)`
  must switch to reasons when they merge.
- An idempotent replay answers in the first request's language.
- Untranslated arguments (`{{reason}}` from parsers) stay English.
