## Why

The review of public #588 (campbooks-cloud#260) found gaps in two checks it
added:

- **The answer-field guard** (`cargo xtask arch`) matched only snake_case
  `secret|token|ticket|password|*url`. It missed camelCase (`clientSecret`,
  `accessToken`), `api_key`, `*_link` and `code`. It also ignored redirects,
  whose `Location` may carry a ticket.
- **The kernel coverage test** counted a policy as admitting every row only
  when its text was `true`, so `USING (1 = 1)` slipped through.

None of these is hit today.

## What Changes

- **Word parts.** The guard reads a field name's words, split at `_`, `-`,
  `.` and camelCase in any case. A field is guarded for:
  - `secret`, `token`, `ticket`, `password`/`passwd` or `code` (and their
    plurals), `apikey` or `otp`;
  - an API, private, access, signing or secret key;
  - `url`, `uri` or `link` as the last word.

  A word that only starts like one (`tokenizer`, `codec`, `keyboard`) is not
  guarded.
- **Redirects.** Every documented 3xx of a mutating operation without
  `secret_once` counts as answering a `Location`, which needs an allow-list
  entry with its reason.
- **New allow-list entries:**
  - `identity.end_session_post`'s `Location`: the app's registered
    post-logout redirect;
  - `identity.register`'s `redirect_uris`: echoed back.
- **Reads stay unchecked.** The idempotency layer stores only mutating
  operations' answers.
- **The coverage test** treats a policy expression as admitting every row
  when its node tree reads no column, calls no function and holds no
  subquery or parameter.

## Impact

- Specs: operation-catalogue (the guard), access (the coverage test).
- Code: `xtask/src/arch.rs`, the server's coverage test. CI only: no runtime
  change, no CHANGELOG.
