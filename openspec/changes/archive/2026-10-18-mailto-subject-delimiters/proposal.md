## Why

The final re-review of #574 (campbooks-cloud#247) left two Lows in the
desktop app's mail links (campbooks-cloud#253):

- A `;` in a subject or body was kept (as `%3B`). An app that decodes first
  and splits on `;` as well as `&` (Python's `parse_qs` before 3.9.2, some
  old CGI parsers) would read `attach=` out of `subject=x;attach=…`.
- A subject or body encoded twice (`%250D%250A`) was kept. An app that
  decodes again would find a line break or another field in it.

## What Changes

- A mail link's subject or body is dropped when, once decoded, it holds a
  `;` (as well as `&` or `?`), or still holds a `%` followed by two hex
  digits. This is the same rule as the address part. A percent sign that is
  text ("50% off") is kept.

## Impact

- Spec: inbox-ui (HTML mail on the desktop app).
- Code: `clients/app/src-tauri/src/links.rs`. Desktop only; nothing changes
  on screen.
