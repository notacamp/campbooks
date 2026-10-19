-- Sent, Archive, Spam and Trash (campbooks-cloud#240): the two facts their
-- lists page on, kept on the conversation by `recount` (domain/conversation.rs)
-- so each view is a keyset page over one partial index, not a look at every
-- conversation's messages.
--
-- Wait at most 10 s for a lock: a busy instance fails this migration fast,
-- and the next start retries it, rather than stalling.
SET LOCAL lock_timeout = '10s';

ALTER TABLE conversations.conversations
    -- When the inbox last wrote in it: the newest `sent_at` of its outbound
    -- messages in neither spam nor the trash. NULL when there is none. Sent
    -- lists every conversation that has one, archived or not.
    ADD COLUMN last_sent_at timestamptz,
    -- Why it is binned: true when it is `trashed` (every message is in the
    -- trash or spam) and at least one message is in spam and not in the
    -- trash. So a trashed conversation is in Spam (`spam`) or in Trash
    -- (`NOT spam`), never both; it is always false when not `trashed`.
    ADD COLUMN spam boolean NOT NULL DEFAULT false,
    ADD CONSTRAINT conversations_spam_is_trashed CHECK (trashed OR NOT spam);

-- seed begin
-- Every conversation stored so far, from its messages, as `recount` works
-- them out. Both tables force row-level security even for their owner, and
-- no workspace is set here: it is lifted for this statement only, inside the
-- migration's transaction. Running it again changes nothing.
ALTER TABLE conversations.conversations NO FORCE ROW LEVEL SECURITY;
ALTER TABLE conversations.messages NO FORCE ROW LEVEL SECURITY;
UPDATE conversations.conversations c
   SET last_sent_at = s.last_sent, spam = c.trashed AND s.in_spam
  FROM (SELECT conversation_id,
               max(sent_at) FILTER (WHERE direction = 'outbound'
                                      AND NOT in_spam AND NOT in_trash) AS last_sent,
               coalesce(bool_or(in_spam AND NOT in_trash), false) AS in_spam
          FROM conversations.messages
         GROUP BY conversation_id) s
 WHERE s.conversation_id = c.id
   AND (c.last_sent_at IS DISTINCT FROM s.last_sent OR c.spam <> (c.trashed AND s.in_spam));
ALTER TABLE conversations.messages FORCE ROW LEVEL SECURITY;
ALTER TABLE conversations.conversations FORCE ROW LEVEL SECURITY;
-- seed end

-- The views' lists have an index each, built concurrently by
-- `…020100_conversations__sent_list_index`, `…020200_conversations__spam_list_index`,
-- `…020300_conversations__trash_list_index` and `…020400_conversations__archive_list_index`.
