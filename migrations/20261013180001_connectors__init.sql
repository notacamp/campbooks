-- The `connectors` module owns the `connectors` schema and nothing else.
-- Other modules' records are referenced by ID only: no cross-schema joins or
-- foreign keys (checked by `cargo xtask arch`).
CREATE SCHEMA IF NOT EXISTS connectors;

-- Row-level security (ADR 0011). Every table holding workspace data carries
-- the workspace and enables RLS keyed on `campbooks.workspace_id`, which the
-- kernel sets on the connection for each request and job:
--
--   CREATE TABLE connectors.example (
--       id uuid PRIMARY KEY,
--       workspace_id text NOT NULL,
--       created_at timestamptz NOT NULL DEFAULT now()
--   );
--   ALTER TABLE connectors.example ENABLE ROW LEVEL SECURITY;
--   CREATE POLICY workspace_isolation ON connectors.example
--       USING (workspace_id = current_setting('campbooks.workspace_id', true));
