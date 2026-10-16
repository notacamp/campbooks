## Purpose

How Campbooks stores the bytes of messages, attachments and files outside
Postgres (ADR 0011): content-addressed per workspace on local disk or an
S3-compatible store, verified on the way in and out, referenced by the records
that hold them, and deleted once nothing does.

## ADDED Requirements

### Requirement: Content-addressed storage per workspace
The system SHALL store a blob under its workspace and the SHA-256 of its
bytes, and SHALL return a reference made of that hash, the size and the
content type given by the uploader. Storing bytes that the workspace already
holds SHALL NOT store them again. Bytes stored in one workspace SHALL NOT be
readable through another workspace.

#### Scenario: The same attachment twice
- **WHEN** the same bytes are stored twice in workspace `ws-1`, once in memory and once as a stream
- **THEN** both calls return the same reference and the store holds one object for them

#### Scenario: Another workspace
- **WHEN** a blob stored in `ws-1` is read through workspace `ws-2` with the same reference
- **THEN** the read fails as not found

### Requirement: Streaming and verification
The system SHALL accept bodies as streams without buffering them whole, SHALL
compute the hash while writing, and SHALL refuse a body whose length or hash
differs from what the uploader said to expect, storing nothing. Reading a blob
SHALL stream it and SHALL end with an error instead of ending cleanly when the
stored bytes do not match the blob's hash or size.

#### Scenario: Truncated download
- **WHEN** a stream that stops short is stored with the full file's expected SHA-256
- **THEN** the call fails with a mismatch and neither an object nor a catalogue row remains

#### Scenario: Broken stream
- **WHEN** the body stream yields an error part-way
- **THEN** the call fails and nothing remains, including any partial upload

#### Scenario: Damaged object
- **WHEN** a stored object's bytes were altered in storage and it is read
- **THEN** the read stream ends with a corruption error naming the hash

### Requirement: Size limit
The system SHALL refuse bodies larger than `CAMPBOOKS_BLOBS__MAX_OBJECT_BYTES`
(default 52,428,800 bytes), whether given in memory or as a stream, and SHALL
store nothing for them.

#### Scenario: One byte over
- **WHEN** the limit is 1,000 bytes and a 1,001-byte stream is stored
- **THEN** the call fails as too large and nothing is stored

### Requirement: Backends
The system SHALL store blobs on local disk under `CAMPBOOKS_BLOBS__PATH`
(default `/data/blobs`) when `CAMPBOOKS_BLOBS__BACKEND` is `fs` (the default),
writing each object atomically, and in an S3-compatible bucket configured by
`CAMPBOOKS_BLOBS__S3__*` when it is `s3`. The server SHALL check that the store
answers before it starts and SHALL refuse to start, naming the setting, when it
does not.

#### Scenario: Missing bucket
- **WHEN** the server starts with `CAMPBOOKS_BLOBS__BACKEND=s3` and a bucket that does not exist
- **THEN** it exits with an error naming the blob storage backend

#### Scenario: Restart on local disk
- **WHEN** a blob is stored on local disk and the store is opened again over the same directory
- **THEN** the blob reads back unchanged

### Requirement: References
A module SHALL record that one of its records holds a blob (attach) and that it
no longer does (detach, or detach everything a record holds) inside its own
database transaction. Attaching a blob the workspace does not hold SHALL fail
without aborting that transaction. Detaching SHALL restart the blob's grace
period.

#### Scenario: Attaching an unknown blob
- **WHEN** a module attaches a reference to bytes never stored in its workspace
- **THEN** the attach fails as not stored and the module's transaction can still commit

### Requirement: Sweeping unreferenced blobs
The system SHALL run a sweep every hour that deletes the objects and catalogue
rows of blobs no record holds once `CAMPBOOKS_BLOBS__SWEEP_GRACE_HOURS`
(default 24) have passed since their last upload or detach, and uploads
abandoned for longer than that. The sweep SHALL NOT delete a blob that is
referenced, nor one that gains a reference or is uploaded again while the
sweep runs.

#### Scenario: Only the unreferenced and old
- **WHEN** one old blob is referenced, one old blob is not, and one new blob is not
- **THEN** the sweep deletes only the old unreferenced blob

#### Scenario: Attach during the sweep
- **WHEN** an attach of an old unreferenced blob is in flight while the sweep runs
- **THEN** the sweep leaves the blob, and once the attach commits the blob is kept for good

#### Scenario: Attach after the sweep claimed the blob
- **WHEN** the sweep has claimed an old unreferenced blob and an attach of it arrives
- **THEN** the attach waits for the sweep, then fails as not stored, and no reference to a deleted blob exists

#### Scenario: Upload during the sweep
- **WHEN** the sweep has claimed an old unreferenced blob and the same bytes are uploaded again
- **THEN** the upload waits for the sweep and the bytes are stored again

### Requirement: Workspace purge
The system SHALL delete every object, catalogue row and reference of a
workspace on request, leaving other workspaces untouched, and repeating the
purge SHALL succeed with nothing left to delete.

#### Scenario: Purging one workspace
- **WHEN** `ws-1` and `ws-2` both hold the same bytes and `ws-1` is purged
- **THEN** `ws-1` holds no blobs and `ws-2` still reads its copy
