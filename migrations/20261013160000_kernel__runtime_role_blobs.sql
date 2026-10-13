-- Let the runtime role (see `kernel__runtime_role`) use the blob tables of
-- `kernel__blobs`. Like the queue and the event log they are infrastructure
-- reached only through the kernel's API, which keys every row by workspace
-- itself (the sweeper works across workspaces), so they carry no row-level
-- security policies.
GRANT SELECT, INSERT, UPDATE, DELETE ON kernel.blobs, kernel.blob_refs TO campbooks_app;
