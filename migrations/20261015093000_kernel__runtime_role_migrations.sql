-- The admin system overview (ADR 0015) shows which schema migrations ran:
-- the runtime role may read sqlx's own bookkeeping table, nothing more.
GRANT SELECT ON _sqlx_migrations TO campbooks_app;
