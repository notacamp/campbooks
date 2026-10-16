# Proposal

## Why

ADR 0011 keeps message bodies (raw MIME), attachments and files out of
Postgres: they bloat backups and the buffer cache. The bytes go to object
storage — local disk for self-hosters, any S3-compatible bucket otherwise —
content-addressed per workspace, so one attachment sent to three inboxes of a
workspace is stored once. Mail sync, documents and file processing (the next
milestones) all need somewhere to put bytes, and a way to delete them when
nothing uses them any more.

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/51.

## What Changes

- **`kernel::blobs`.** A `BlobStore` in `AppState` (`state.blobs`):
  `put` (bytes or a stream, hashed while written, never buffered whole) →
  `BlobRef` (SHA-256, size, content type); `put_expecting` rejects a body
  whose size or hash differs from what the caller knows (a truncated
  download); `get` streams the bytes back, verified against the hash;
  `head`. Objects are keyed `<workspace>/<aa>/<sha256>`.
- **Backends** chosen with `CAMPBOOKS_BLOBS__BACKEND`: `fs` (default,
  `CAMPBOOKS_BLOBS__PATH=/data/blobs`; atomic writes with fsync) and `s3`
  (`CAMPBOOKS_BLOBS__S3__BUCKET`, `__ENDPOINT`, `__REGION`,
  `__ACCESS_KEY_ID`, `__SECRET_ACCESS_KEY`, `__PATH_STYLE`; streamed
  multipart uploads). An in-memory backend for tests. Built on the
  `object_store` crate.
- **References.** `kernel.blobs` (one row per stored object) and
  `kernel.blob_refs` (which record holds which blob, by type and id).
  Modules `attach`/`detach`/`detach_owner` inside their own transactions.
- **Sweeper.** The kernel schedules `kernel.sweep_blobs` hourly on the job
  queue: it deletes blobs with no reference once
  `CAMPBOOKS_BLOBS__SWEEP_GRACE_HOURS` (default 24) have passed since their
  last upload or detach, and abandoned uploads. It never deletes a blob that
  gained a reference meanwhile.
- **Workspace deletion.** `purge_workspace` deletes a workspace's objects,
  rows and references; the `workspaces` module calls it on
  `WorkspaceDeleting`.
- **Limits.** `CAMPBOOKS_BLOBS__MAX_OBJECT_BYTES` (default 50 MiB). Content
  types are stored as given (normalized), never trusted.
- **Deploy.** A `blobs-data` volume at `/data/blobs` in `deploy/compose.yaml`;
  the image creates the directory for its non-root user. The server checks
  the store answers on startup. S3 settings, encryption at rest and backups
  documented in `deploy/README.md` and `.env.example`.

## Capabilities

### New Capabilities

- `blob-storage`: storing, reading, referencing, sweeping and purging the
  bytes of messages, attachments and files.

### Modified Capabilities

- `events-and-jobs` is unchanged in behaviour; the kernel registers one more
  schedule (`kernel.sweep_blobs`) next to `kernel.prune_jobs`.

## Impact

- Modules: `campbooks-kernel` (new `blobs`, `BlobsConfig`, `AppState.blobs`,
  test helpers), `campbooks-server` (builds and probes the store on startup).
  No domain module changes, no new module, no cross-module dependency, no
  `architecture.toml` entry.
- Data: schema `kernel` gains `blobs` and `blob_refs` (migration
  `20261013151100_kernel__blobs.sql`), owned by the kernel.
- Dependencies: `object_store` 0.14.2 (MIT/Apache-2.0, features `fs` and
  `aws`), plus `bytes` and `futures-util`, already in the graph. SHA-256
  comes from `aws-lc-rs`, already the TLS and JWT crypto backend.
- Self-hosters: a new `blobs-data` volume (created by `docker compose up`);
  optional `CAMPBOOKS_BLOBS__*` settings. Back the volume up with Postgres.
- OpenAPI: unchanged (no endpoint).

## Non-goals

- **Download or upload endpoints.** Blobs are never served by URL from the
  kernel; modules add their own endpoints with access checks.
- **Encryption at rest by Campbooks.** Another change introduces
  `CAMPBOOKS_SECRET_KEY`; until then, storage-level encryption (bucket SSE,
  encrypted disk) is documented.
- **Quotas** per workspace or plan, and storage usage reporting.
- **Migrating between backends** with a command; the key layout is identical,
  so a copy works (documented).
- **Deduplication across workspaces** — deliberately not done, so one
  workspace's data never depends on another's.
- **Calling `purge_workspace`**: the `workspaces` module (issue #18) does it.
