-- Scout proposes changes in a chat (campbooks-cloud#305, phase 2): each of
-- Scout's answers holds the proposals it made, at most three, each a card
-- the member confirms or declines. `[{id, kind, action, record, facts,
-- detail, request, undo, sentence, intent, state}]`, built by the server
-- from the catalogue and the record's own facts; the model's words appear
-- only as a label name, a rule sentence or a reply intent. Nothing here is
-- ever run by the server: the member's own request makes the change.
-- A new column with a constant default only (no table rewrite); the
-- timeout keeps an unexpected lock wait from holding up a deploy.
SET LOCAL lock_timeout = '5s';

ALTER TABLE assistant.chat_messages ADD COLUMN proposals jsonb NOT NULL DEFAULT '[]';
