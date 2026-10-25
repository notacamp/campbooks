-- Microsoft mail (microsoft-mail-connection, campbooks-cloud#315): a fourth
-- provider, Outlook.com, Hotmail, Live, MSN and Microsoft 365 through
-- Microsoft Graph, connected through the front door like Gmail. Its tokens
-- live in `connectors.credentials` sealed like Google's, and its mailbox is
-- a `messages` stream whose `mailbox_key` is the Graph user id: no new table,
-- so no new row-level security, grant or foreign key.
--
-- Wait at most 5 s for a lock: a busy instance fails this migration fast,
-- and the next start retries it, rather than stalling.
SET LOCAL lock_timeout = '5s';

ALTER TABLE connectors.connections DROP CONSTRAINT IF EXISTS connections_provider_check;
ALTER TABLE connectors.connections
    ADD CONSTRAINT connections_provider_check
    CHECK (provider IN ('google', 'zoho', 'imap', 'microsoft'));
