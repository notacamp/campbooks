# Tasks

## 1. Schema

- [x] 1.1 Migration `20261013151100_kernel__blobs.sql`: `kernel.blobs` (workspace, hash, size, `touched_at`) and `kernel.blob_refs` (owner type and id, foreign key to the blob); verify `cargo xtask arch` accepts it

## 2. Store

- [x] 2.1 `BlobHash`, `BlobRef`, `BlobOwner`, `BlobBody`, `Expected`, `BlobError` (with `kernel::Error` and `JobError` conversions), workspace / owner / content-type checks; verify with unit tests
- [x] 2.2 `BlobsConfig` (`CAMPBOOKS_BLOBS__*`) with startup checks naming the bad setting; verify defaults, parsing, messages, secret redaction
- [x] 2.3 Backends through `object_store`: `fs` (fsync), `s3` (path-style or virtual-hosted, plain HTTP for private stores), in-memory for tests; `probe` on startup
- [x] 2.4 `put` (bytes; stream to a staging key with multipart, hashed and size-limited, then renamed into place), `put_expecting`, `get` (verified stream), `head`; verify roundtrips (memory, disk), dedupe, truncated stream, size limit, damaged object
- [x] 2.5 `attach`, `detach`, `detach_owner` in the caller's transaction; verify an unknown blob fails without breaking the transaction and a detach restarts the grace period

## 3. Deletion

- [x] 3.1 `sweep` (claim `FOR UPDATE SKIP LOCKED`, re-check, delete rows and objects in one transaction; abandoned staging uploads by age) and the hourly `kernel.sweep_blobs` schedule; verify only unreferenced blobs past their grace go, an attach or put racing the sweep never loses a blob, the job runs on the worker
- [x] 3.2 `purge_workspace`; verify it deletes one workspace's blobs only and is idempotent, on memory and disk

## 4. Server and deploy

- [x] 4.1 `AppState.blobs` (`with_blobs`), built from config and probed by the server on startup; `testing::state` uses the in-memory store
- [x] 4.2 Image creates `/data/blobs` for `nonroot`; `blobs-data` volume in `deploy/compose.yaml`; `.env.example`, `deploy/README.md` (S3, encryption at rest, backups), `compose.dev.yaml`
- [x] 4.3 Run the `s3` backend once against a throwaway S3-compatible container (ignored test `s3_backend_against_a_live_store`)

## 5. Follow-ups (not in this change)

- [x] 5.1 Encryption at rest with `CAMPBOOKS_SECRET_KEY` (envelope keys per workspace) — moved to campbooks-cloud#176
- [x] 5.2 `workspaces` (#18) calls `purge_workspace` on `WorkspaceDeleting` — shipped: the `workspaces.purge_workspace` job calls `BlobStore::purge_workspace`
- [x] 5.3 Per-workspace storage usage and quotas — moved to campbooks-cloud#178
- [x] 5.4 A first download endpoint (in the owning module) streaming `get` with `Content-Disposition: attachment` and `nosniff`; `413` for over-limit uploads — moved to campbooks-cloud#179
- [x] 5.5 Backups and restore drill including object storage (#54) — shipped: cloud#54; `blobs-data` backup in `deploy/README.md`

## 6. Wrap-up

- [x] 6.1 `docs/architecture.md` updated
- [x] 6.2 OpenAPI unchanged (`cargo xtask openapi --check`); client types unchanged (no endpoint)
- [x] 6.3 CHANGELOG `[Unreleased]` updated
- [x] 6.4 Gates green: fmt, clippy, tests, `cargo xtask arch`, `cargo xtask openapi --check`, sqlx prepare check, `cargo deny`, `cargo machete`, `openspec validate --all --strict`
- [x] 6.5 UI: none (no client change)
