-- Let the runtime role (created by `kernel__runtime_role`) use the system
-- module's tables. Instance-wide data: no row-level security.
GRANT USAGE ON SCHEMA system TO campbooks_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON system.instance TO campbooks_app;
