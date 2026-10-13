-- Postgres extensions the platform relies on (ADR 0016), installed once per
-- database. Both ship with Postgres itself (contrib, in the official and
-- pgvector images) and are trusted extensions, so the database owner that
-- runs migrations may create them without being a superuser.
--
-- - `unaccent`: accent-insensitive text search ("joao" finds "João").
-- - `pg_trgm`: trigram indexes for partial matches on names and addresses.
--
-- Managed Postgres services list both among their allowed extensions. If an
-- install refuses them, this migration fails here, naming the extension.
CREATE EXTENSION IF NOT EXISTS unaccent;
CREATE EXTENSION IF NOT EXISTS pg_trgm;
