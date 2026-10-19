-- When the newest message you sent in a conversation was sent, for the Sent
-- view (campbooks-cloud#240). Kept by `recount`, with the other counts and
-- flags; NULL while you wrote none (or all you wrote is in the trash or spam).
ALTER TABLE conversations.conversations ADD COLUMN last_sent_at timestamptz;

UPDATE conversations.conversations c
   SET last_sent_at = m.last
  FROM (SELECT conversation_id, max(sent_at) AS last
          FROM conversations.messages
         WHERE direction = 'outbound' AND NOT in_trash AND NOT in_spam
         GROUP BY conversation_id) m
 WHERE m.conversation_id = c.id;
