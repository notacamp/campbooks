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
  header (CR, LF, `&`, `;`, `?`, `#`, `<`, `>`, `"`, `\`, a leftover `%`).
  Anything else opens nowhere.

## Impact

- Spec: inbox-ui (HTML mail on the desktop app).
- Code: `src-tauri/src/links.rs`. Nothing changes on screen.
