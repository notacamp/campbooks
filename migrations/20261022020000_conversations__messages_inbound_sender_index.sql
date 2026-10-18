-- no-transaction
-- An inbox's received mail by sender, newest first, spam included: whether a
-- sender's own mail was reported, is under review or was found suspicious
-- (`conversations.list_senders` with `reachable`, Forward's picker, on every
-- keystroke; campbooks-cloud#246). Without it that check walked the inbox's
-- whole history. Built CONCURRENTLY, one statement per migration, like
-- `messages_inbound_recent_idx` (see there).
CREATE INDEX CONCURRENTLY IF NOT EXISTS messages_inbound_sender_idx
    ON conversations.messages (inbox_id, sender_address, sent_at DESC)
    WHERE direction = 'inbound';
