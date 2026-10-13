# server-i18n Specification

## Purpose
How the server speaks the caller's language (en, pt-PT, es-ES, fr) in the
messages a person reads: problem titles and details, emails and notices.

## Requirements

### Requirement: Request locale
The server SHALL resolve one locale per request among `en`, `pt-PT`,
`es-ES` and `fr`, always as the full tag. It SHALL use, in order: the
calling user's saved locale; the first `Accept-Language` entry (by
descending `q`, ignoring `q=0` and `*`) whose primary language subtag is
`en`, `pt`, `es` or `fr`, mapped to `en`, `pt-PT`, `es-ES` or `fr`; and
otherwise `en`. Client tokens have no saved locale. A user with no saved
locale SHALL be treated like an anonymous caller.

#### Scenario: Brazilian Portuguese falls back to European Portuguese
- **WHEN** an anonymous request sends `Accept-Language: pt-BR, en;q=0.5`
- **THEN** the request locale is `pt-PT`

#### Scenario: Unsupported languages are skipped
- **WHEN** an anonymous request sends `Accept-Language: de-DE, fr;q=0.3`
- **THEN** the request locale is `fr`

#### Scenario: The saved locale wins
- **WHEN** a user whose saved locale is `es-ES` calls with `Accept-Language: fr`
- **THEN** the request locale is `es-ES`

#### Scenario: No preference at all
- **WHEN** an anonymous request sends no `Accept-Language`
- **THEN** the request locale is `en`

### Requirement: Localized problem details
Every problem+json response SHALL carry a stable machine-readable `code`
(an English snake_case identifier that never changes with the locale) and
SHALL render `title` and `detail` in the request locale, with a
`Content-Language` header naming it. `type` SHALL NOT change with the
locale. When the request does not reach the localization layer, the problem
SHALL be complete in English.

#### Scenario: A conflict in Portuguese
- **WHEN** a user whose saved locale is `pt-PT` tries to leave a workspace of which they are the last owner
- **THEN** the response is 409 with `code` `last_owner`, a Portuguese `title` and `detail`, and `Content-Language: pt-PT`

#### Scenario: Token errors before the user is known
- **WHEN** a request with an invalid token sends `Accept-Language: fr`
- **THEN** the response is 401 with `code` `invalid_token`, a French `title` and `detail`, and `Content-Language: fr`

#### Scenario: A 404 outside the API
- **WHEN** an unknown path is requested with `Accept-Language: es`
- **THEN** the problem has `code` `not_found`, a Spanish `title`, and `Content-Language: es-ES`

### Requirement: Server message catalogues at parity
Server text a user reads SHALL come from message catalogues: one directory
`i18n/` per module (and one for the kernel) holding `en.json`,
`pt-PT.json`, `es-ES.json` and `fr.json` in the client's format (nested
keys, `{{name}}` placeholders, CLDR plural suffixes). A CI check SHALL fail
when a directory lacks a locale or has an extra file, a file is not valid
JSON of nested strings, a locale lacks or adds a message, a value is empty,
a plural message lacks `_one` or `_other` or has a form its locale does not
use, or a message's placeholders differ from the English ones.

#### Scenario: A missing translation fails CI
- **WHEN** `crates/modules/workspaces/i18n/fr.json` lacks `errors.last_owner`
- **THEN** `cargo xtask i18n` fails naming the file and the key

#### Scenario: A placeholder dropped in translation
- **WHEN** the Spanish text of a message omits the `{{max}}` placeholder the English text has
- **THEN** `cargo xtask i18n` fails naming the file, the key and the placeholder

### Requirement: Rendering outside a request
The kernel SHALL render a catalogue message for an explicit locale and
arguments, choosing the plural form from a `count` argument by the locale's
CLDR cardinal rules and falling back to the `other` form, then to English,
then to the key, without failing. Background jobs that render text SHALL
carry the recipient's locale in their payload.

#### Scenario: Plural forms follow the locale
- **WHEN** a message with `_one` and `_other` forms is rendered with `count` 0
- **THEN** French uses the `_one` form and European Portuguese and English use the `_other` form

#### Scenario: A missing translation degrades
- **WHEN** a key exists in `en` but not in the requested locale's loaded catalogue
- **THEN** the English text is returned
