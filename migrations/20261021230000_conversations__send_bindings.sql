-- Sends and change requests bound to the request that made them, with MACs
-- under a key the database never holds (campbooks-cloud#227).
--
-- Any transaction working in a workspace can write these rows, so a row is
-- no proof that a member asked. Each now carries a MAC, keyed by a key
-- derived from the instance secret (never stored here), that SQL cannot
-- forge:
--
-- - `drafts.request_mac`: the author's request to send (`send_draft`), over
--   the workspace, draft, send generation, author, inbox and a hash of
--   everything that goes out. `release_send` verifies it before it writes
--   anything, and clears it once the send is released or cancelled.
-- - `drafts.send_mac`: the released send (`release_send`), over the
--   workspace, send id, inbox, author and the outgoing document. The
--   outbound-requests port verifies it before the connector sends.
-- - `change_requests.mac`: a change request, over the workspace, request,
--   inbox, conversation and items. The port verifies it before any item
--   stands.
--
-- A NULL MAC never verifies. Drafts still `scheduled` when this runs were
-- scheduled by the previous version, so they have none and would never be
-- released: they go back to drafts here, and their authors send them again
-- (their release jobs then find nothing to do). A draft already `sending`
-- has no `send_mac` either: its send is not made, and the draft fails (as
-- an unconfirmed send) so it can be sent again; see the change's design.
SET LOCAL lock_timeout = '10s';

ALTER TABLE conversations.drafts
    ADD COLUMN request_mac bytea,
    ADD COLUMN send_mac bytea;
ALTER TABLE conversations.change_requests ADD COLUMN mac bytea;

-- Both tables force row-level security even for their owner, and no
-- workspace is set here: it is lifted for this statement only, inside the
-- migration's transaction.
ALTER TABLE conversations.drafts NO FORCE ROW LEVEL SECURITY;
UPDATE conversations.drafts
   SET state = 'draft', send_at = NULL, updated_at = now()
 WHERE state = 'scheduled';
ALTER TABLE conversations.drafts FORCE ROW LEVEL SECURITY;
