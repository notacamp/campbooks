-- Kernel infrastructure: the catalogue of stored blobs and who references
-- them (ADR 0011). The bytes live in object storage (local disk or an
-- S3-compatible bucket) under `<workspace_id>/<aa>/<sha256>`; Postgres keeps
-- one row per stored object and one row per reference, so the sweeper can
-- delete what nothing references any more. Written only through
-- `campbooks_kernel::blobs`.

-- One row per object in the store: a workspace's copy of some bytes.
CREATE TABLE kernel.blobs (
    workspace_id text NOT NULL,
    -- SHA-256 of the bytes, lower-case hex: the object's address.
    hash text NOT NULL CHECK (hash ~ '^[0-9a-f]{64}$'),
    size bigint NOT NULL CHECK (size >= 0),
    created_at timestamptz NOT NULL DEFAULT now(),
    -- Last put of these bytes or last detach of a reference. The sweeper only
    -- deletes a blob with no references whose `touched_at` is older than the
    -- grace period, so a fresh put has time to be attached and a detach can
    -- be undone.
    touched_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (workspace_id, hash)
);

-- The sweeper walks unreferenced candidates oldest first.
CREATE INDEX blobs_touched_at_idx ON kernel.blobs (touched_at);

-- One row per record holding a blob. A module attaches and detaches its
-- records inside its own transactions; other modules' records are named by
-- type and id only.
CREATE TABLE kernel.blob_refs (
    workspace_id text NOT NULL,
    hash text NOT NULL,
    -- The holding record: its type, `<module>.<record>`, and its id.
    owner_type text NOT NULL,
    owner_id uuid NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (workspace_id, hash, owner_type, owner_id),
    -- A reference keeps its blob's row (and so its object) alive: the sweeper
    -- can never delete a referenced blob, even by mistake.
    FOREIGN KEY (workspace_id, hash) REFERENCES kernel.blobs (workspace_id, hash)
);

-- Detaching everything a record holds.
CREATE INDEX blob_refs_owner_idx ON kernel.blob_refs (workspace_id, owner_type, owner_id);
