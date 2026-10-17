-- Outbound changes may put a message in the trash or spam and take it out
-- again (campbooks-cloud#169): `conversations.changes_requested` v1 gains the
-- kinds `trash`, `untrash`, `spam` and `unspam`.
-- The CHECK constraint widens, and a change that put a message in the trash
-- at Zoho keeps the folder it was in (`restore_to`: Zoho's location tokens,
-- ids only), so untrash can put it back there. The
-- widened check is added NOT VALID: swapping it takes the table's lock only
-- briefly; the next migration validates it in a transaction of its own. The timeout keeps an unexpected
-- lock wait from holding up a deploy.
SET LOCAL lock_timeout = '5s';

ALTER TABLE connectors.outbound_changes DROP CONSTRAINT IF EXISTS outbound_changes_kind_check;
ALTER TABLE connectors.outbound_changes ADD CONSTRAINT outbound_changes_kind_check
    CHECK (kind IN ('mark_read', 'mark_unread', 'star', 'unstar', 'archive', 'unarchive',
                    'trash', 'untrash', 'spam', 'unspam', 'add_label', 'remove_label',
                    'move_to_stream', 'unsupported')) NOT VALID;

-- Nullable, no default: adding it rewrites nothing. A sync clears it once a
-- message leaves the trash; the partial index finds the few that hold one.
ALTER TABLE connectors.outbound_changes ADD COLUMN IF NOT EXISTS restore_to text[];
CREATE INDEX IF NOT EXISTS outbound_changes_restore_idx ON connectors.outbound_changes (stream_id)
    WHERE restore_to IS NOT NULL;
