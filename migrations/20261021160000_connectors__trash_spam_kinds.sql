-- Outbound changes may put a message in the trash or spam and take it out
-- again (campbooks-cloud#169): `conversations.changes_requested` v1 gains the
-- kinds `trash`, `untrash`, `spam` and `unspam`.
-- Only the CHECK constraint changes. The widened check is added NOT VALID:
-- swapping it takes the table's lock only briefly; the next migration
-- validates it in a transaction of its own. The timeout keeps an unexpected
-- lock wait from holding up a deploy.
SET LOCAL lock_timeout = '5s';

ALTER TABLE connectors.outbound_changes DROP CONSTRAINT outbound_changes_kind_check;
ALTER TABLE connectors.outbound_changes ADD CONSTRAINT outbound_changes_kind_check
    CHECK (kind IN ('mark_read', 'mark_unread', 'star', 'unstar', 'archive', 'unarchive',
                    'trash', 'untrash', 'spam', 'unspam', 'add_label', 'remove_label',
                    'move_to_stream', 'unsupported')) NOT VALID;
