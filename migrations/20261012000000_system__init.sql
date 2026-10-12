-- The `system` module owns the `system` schema and nothing else.
CREATE SCHEMA IF NOT EXISTS system;

-- One row per Campbooks instance; proves the per-module schema convention
-- and gives the instance a stable identifier.
CREATE TABLE system.instance (
    id uuid PRIMARY KEY,
    created_at timestamptz NOT NULL DEFAULT now()
);
