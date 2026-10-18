## Why

An upload over `CAMPBOOKS_BLOBS__MAX_OBJECT_BYTES` surfaced as 400
`file_too_large`, and a JSON body over its limit as 400 `invalid_body`
(campbooks-cloud#179, from the blob-storage change's task 5.4). A client cannot
tell "too big" from "malformed" by the status, and HTTP has 413 for exactly
this.

## What Changes

- The kernel's `Error` gains `PayloadTooLarge(Reason)`, answered 413
  problem+json.
- `BlobError::TooLarge` maps to it: 413 `file_too_large` with the limit in the
  detail (all four locales already name it).
- The kernel's `Json` extractor answers a body over its limit 413
  `body_too_large`, a new message in en, pt-PT, es-ES and fr. Other rejections
  stay 400.
- Uploads ready for the first upload route (compose attachments): its handler
  returns the blob store's error as is.

## Impact

- Spec: blob-storage (Size limit).
- Code: kernel `error`, `blobs`, `extract`, the kernel locales.
- No migration.
