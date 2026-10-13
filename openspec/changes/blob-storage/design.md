# Design

## Context

ADR 0011 decides that bytes live outside Postgres, content-addressed per
workspace, on local disk or any S3-compatible store. The kernel has a
durable job queue (ADR 0012) to run maintenance on, and `AppState` is what
handlers and jobs reach. No module stores bytes yet; mail sync is the first
consumer.

## Data ownership and events

- **Schema `kernel`**, owned by `campbooks-kernel`, written only through
  `kernel::blobs`. Migration `20261013151100_kernel__blobs.sql`.
- **Objects** live in the configured store under `<workspace>/<aa>/<sha256>`
  (`aa` = the hash's first two hex digits, to keep directories small on
  disk), plus in-flight uploads under `_staging/<uuid>`.
- **Events published or consumed:** none. `purge_workspace` is a method the
  `workspaces` module calls from its own `WorkspaceDeleting` handler; the
  kernel knows no domain event.
- **Endpoints:** none, so no OAuth scope. Blobs are never served by URL by
  the kernel; a module's download endpoint checks that the caller may see the
  record (404 otherwise) and streams `get`.
- **AI:** none.

### Tables

`kernel.blobs` — one row per stored object.

| Column | Type | Notes |
| --- | --- | --- |
| `workspace_id` | `text` | PK part |
| `hash` | `text` | PK part; 64 lower-case hex (CHECK) |
| `size` | `bigint` | bytes |
| `created_at` | `timestamptz` | |
| `touched_at` | `timestamptz` | last put or detach; the grace period counts from here |

Index `(touched_at)` for the sweeper.

`kernel.blob_refs` — one row per record holding a blob.

| Column | Type | Notes |
| --- | --- | --- |
| `workspace_id`, `hash` | `text` | FK → `kernel.blobs` |
| `owner_type` | `text` | `<module>.<record>`, e.g. `mail.message` |
| `owner_id` | `uuid` | the record's id |
| `created_at` | `timestamptz` | |

PK `(workspace_id, hash, owner_type, owner_id)`; index `(workspace_id,
owner_type, owner_id)` for `detach_owner`. The foreign key stays inside the
kernel's schema and makes deleting a referenced blob impossible even by
mistake.

## Decisions

### API

```rust
let blob: BlobRef = state.blobs.put(ws, BlobBody::stream(s), "message/rfc822").await?;
let blob = state.blobs.put_expecting(ws, body, ct, Expected { hash, size }).await?;
let bytes = state.blobs.get(ws, &blob).await?;          // Stream<Result<Bytes, BlobError>>
let info = state.blobs.head(ws, &blob.hash).await?;     // Option<BlobInfo>
state.blobs.attach(&mut tx, ws, &blob, &BlobOwner::new("mail.message", id)).await?;
state.blobs.detach(&mut tx, ws, &blob.hash, &owner).await?;
state.blobs.detach_owner(&mut tx, ws, &owner).await?;
state.blobs.purge_workspace(ws).await?;
```

The workspace is a `&str` like everywhere in the kernel (jobs, events);
it must be 1–64 ASCII letters, digits or hyphens, so it is a safe key
segment and `_staging` can never collide with one. `BlobRef` serializes as
`{hash, size, content_type}` so a module can keep it in a row or a JSON
column.

### `object_store` rather than an S3 client of our own

`object_store` 0.14.2 (Apache Arrow project, MIT/Apache-2.0, released
2026-09-15) gives local disk, S3-compatible and in-memory stores behind one
trait, streaming multipart uploads (`BufWriter`: a single PUT under one part,
multipart above), server-side copy/rename, batched deletes, and on local disk
atomic writes with `fsync` of the file and its directory. Its `aws` feature
signs with `aws-lc-rs` and talks through `reqwest` 0.13 — both already our
stack — so it adds `quick-xml`, `crc-fast`, `md-5`, `humantime`, `walkdir`,
`itertools` and little else. Hand-rolling SigV4, multipart upload, retries
and the error mapping would be a few hundred lines of security-sensitive code
to own for no gain. Passes `cargo deny`.

### Writing: hash while streaming, then move into place

The key is the hash, which a stream only reveals at its end. A streamed
`put` writes to `_staging/<uuid>` through a multipart upload (8 MiB parts,
two in flight, so memory stays bounded however large the file), hashing and
counting every chunk, refusing the body as soon as it exceeds
`MAX_OBJECT_BYTES` or the expected size, and checking the expected hash at
the end. Any failure aborts the upload and deletes the staging object, so a
truncated download never becomes a blob. Then it upserts the catalogue row
and renames the staging object to its key (or drops it, if the bytes are
already stored). Bytes already in memory are hashed first and written
straight to their key, skipping staging. On local disk the rename is atomic;
on S3 it is a server-side copy and delete.

`get` wraps the object's stream: it hashes what it reads and ends with
`BlobError::Corrupt` instead of ending cleanly if the bytes, or their
length, do not match the address, so damaged storage is never served as
good data without notice.

### Deleting safely (mark, lock, delete)

The catalogue row exists before its object: `put` upserts the row
(`touched_at = now()`) and only then writes the object. The sweeper works
from rows, in batches of 100, each batch one transaction:

1. `SELECT … FOR UPDATE SKIP LOCKED` rows with no reference and
   `touched_at` older than the grace period.
2. In a second statement (a fresh snapshot), delete those of them that
   still have no reference and are still old enough.
3. Delete their objects (already-missing counts as deleted); commit.

Races:

- **Attach vs sweep.** Attach takes `FOR KEY SHARE` on the row before
  inserting the reference (the FK takes it too). If attach holds it, the
  sweeper skips the row. If the sweeper holds it, attach waits; once the
  sweep commits, attach finds no row and returns `NotStored` without
  aborting the caller's transaction (no FK violation is ever raised). The
  second statement's fresh snapshot sees any reference committed before the
  lock was taken. A reference therefore never points at a deleted object.
- **Put vs sweep.** A put of bytes being swept waits on the row lock, then
  inserts a fresh row and writes the object again. A put that touched the row
  first makes it ineligible for the whole grace period — ample time to finish
  writing the object and attach it.
- **Crashes.** A crash leaves at worst a row without an object (swept after
  its grace period, missing objects count as deleted) or a staging upload
  (swept by age). Incomplete S3 multipart uploads are aborted on error; a
  bucket lifecycle rule for crashed ones is documented.

No transaction is held open while bytes are uploaded; the sweeper holds its
row locks only while deleting a batch of objects.

`detach` (and `detach_owner`) also set `touched_at = now()`, so the grace
period counts from the last use: a detach (an undone AI proposal, a message
moved to trash and restored) can be reversed by attaching again within it.

### Workspace deletion

`purge_workspace` lists and deletes every object under `<workspace>/`, then
deletes the workspace's references and rows in one transaction. Objects go
first so a failure half-way leaves rows that a re-run finishes; it is
idempotent. The `workspaces` module calls it from its `WorkspaceDeleting`
handler; nothing calls it yet.

### Configuration and startup

`CAMPBOOKS_BLOBS__*` is a `BlobsConfig` section of the kernel `Config`. The
server builds the store before serving (creating the `fs` directory if
needed) and lists `_staging/` once as a probe, so a missing bucket, a wrong
key or an unwritable path stops startup with a message naming the setting.
`AppState::new` leaves `blobs` unconfigured (every call fails with
`NotConfigured`); the server attaches the real store with `with_blobs`, and
`testing::state` an in-memory one. The sweep job is a no-op in a process
without a store.

### Limits and content types

`MAX_OBJECT_BYTES` (default 50 MiB) is enforced while streaming.
Content types are trimmed and lower-cased if they look like
`type/subtype[; params]` in printable ASCII, otherwise stored as
`application/octet-stream`. They are recorded on the `BlobRef`, not on the
object (dedupe makes a per-object type meaningless), and never trusted: a
download endpoint serves `Content-Disposition: attachment` with `nosniff`.

### Encryption at rest

Not in this change: `CAMPBOOKS_SECRET_KEY` arrives with another change, and
envelope encryption per workspace would build on it. Content addressing then
needs care (hash the plaintext, encrypt with a per-workspace key, so dedupe
within a workspace still works). Until then, `deploy/README.md` documents
storage-level encryption: bucket SSE, or an encrypted disk under the volume.

## Risks and trade-offs

- A deployment that loses the bucket but keeps Postgres has rows pointing at
  nothing; `get` returns `NotFound`. Backups must take both (documented).
- `object_store` is a sizeable dependency; it is maintained by the Arrow
  project and replaces code we would otherwise write and secure ourselves.
- Listing `_staging/` each sweep costs one list call per hour.
