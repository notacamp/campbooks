-- Validate the check `…020200_attention__worth_a_look` widened NOT VALID
-- (campbooks-cloud#274), in a transaction of its own.

-- Never wait long for a lock on a busy table: fail and retry the deploy.
SET LOCAL lock_timeout = '5s';

ALTER TABLE attention.items VALIDATE CONSTRAINT items_reason_check;
