## Why

The re-review of #566 (campbooks-cloud#243) found that a desktop mail link's
address part, before `?`, reached the mail app as written:
`mailto:a@b.example%0D%0Abcc:evil@x` could carry a header to a mail app that
decodes the whole link before splitting it (campbooks-cloud#247). It cannot
carry a file (`attach` only travels as a filtered field), so this is
defence in depth.

## What Changes

- A mail link opens only when its address part is nothing but addresses:
  none, or comma-separated `local@domain`s, decoded, with no control
  character, blank, or delimiter a mail app could read as another field or
  header (CR, LF, `&`, `;`, `?`, `#`, `<`, `>`, `"`, `\`, a leftover `%`),
  valid UTF-8 with no Unicode control (C0, C1) or format character (bidi
  overrides, zero-width characters); other Unicode is kept for
  internationalised addresses. A link with an authority
  (`mailto://user:password@host`) is refused too. Anything else opens
  nowhere.

## Impact

- Spec: inbox-ui (HTML mail on the desktop app).
- Code: `src-tauri/src/links.rs`. Nothing changes on screen.
