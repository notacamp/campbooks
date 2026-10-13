-- The `Content-ID` of an inline part (angle brackets stripped): what the
-- HTML body's `cid:` images name, so `get_message_body` can show them.
-- Nullable: parts without one, and messages stored before this column.
ALTER TABLE conversations.attachments ADD COLUMN content_id text;
