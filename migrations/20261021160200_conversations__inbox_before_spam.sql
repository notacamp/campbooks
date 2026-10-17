-- Report as spam takes a conversation's messages out of the inbox
-- (campbooks-cloud#169); "not spam" puts each back in the inbox only when it
-- was there before. This remembers that, per message, until "not spam" or
-- the next report. Nullable, no default: adding it rewrites nothing. The
-- timeout keeps an unexpected lock wait from holding up a deploy.
SET LOCAL lock_timeout = '5s';

ALTER TABLE conversations.messages ADD COLUMN IF NOT EXISTS inbox_before_spam boolean;
