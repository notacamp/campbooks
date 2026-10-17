-- The outgoing document a released send was written as (campbooks-cloud#226).
--
-- `conversations.release_send` writes the outgoing-message document, holds
-- its blob for the draft and publishes `conversations.send_requested`
-- naming it. The connector now asks this module, through the kernel's
-- outbound-requests port, for the document of a send before it sends, and
-- sends the one recorded here rather than the one an event names: any
-- transaction working in the workspace can append that event. Set together
-- with `send_id` when a draft becomes `sending`; a draft released before
-- this column existed has none, and its send is not made (the draft fails
-- with `unknown_outcome` after the hour `conversations.check_send` waits,
-- and can be sent again).
SET LOCAL lock_timeout = '10s';

ALTER TABLE conversations.drafts ADD COLUMN send_document jsonb;
