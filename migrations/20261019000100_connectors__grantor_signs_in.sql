-- Whether the member who connected a mailbox also signs in with it (their
-- own mailbox, not a shared one): decides who may keep its mail when it is
-- disconnected. Every connection so far came with a way in (sign-up or a
-- connect, which added one), so existing rows are true.
ALTER TABLE connectors.connections
    ADD COLUMN IF NOT EXISTS grantor_signs_in boolean NOT NULL DEFAULT true;
