-- Taught rules bound to the member who taught them, with a MAC under a key
-- the database never holds (campbooks-cloud#242).
--
-- Any transaction working in a workspace can insert a rule or change one's
-- conditions, actions or inbox, and the next inbound mail would run it: the
-- changes it makes at the provider are signed by the server. Each rule now
-- carries a MAC (`rules.mac`) over the workspace, the rule, its binding
-- generation (`rules.binding_generation`, one more at every signing), its
-- inbox, who taught it, its state, its precedence and a hash of its
-- conditions and actions. Only a member's request signs it; a rule acts only
-- when it verifies. A NULL MAC never verifies.
--
-- The rules kept when this runs (active or paused) have none. They are
-- signed once by `conversations.resign_rules`, and only while they are still
-- as they were here: this migration records each one's id and a SHA-256 of
-- its bound fields in `conversations.rule_resigns`, which the runtime role
-- may read but never write. `conversations.take_rule_resign` (a definer
-- function owned by `campbooks_definer`) removes a rule's row and says
-- whether the rule still hashes the same, with the same expression; the job
-- signs it only then. A rule changed since, or planted later, waits for a
-- member to confirm it. See the change's design.md.
SET LOCAL lock_timeout = '10s';

-- Catalog-only: a constant default and a nullable column.
ALTER TABLE conversations.rules
    ADD COLUMN mac bytea,
    ADD COLUMN binding_generation bigint NOT NULL DEFAULT 0;

CREATE TABLE conversations.rule_resigns (
    rule_id uuid PRIMARY KEY REFERENCES conversations.rules (id) ON DELETE CASCADE,
    workspace_id uuid NOT NULL,
    -- SHA-256 of the rule's bound fields as stored when this ran.
    content_hash bytea NOT NULL CHECK (octet_length(content_hash) = 32),
    created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX rule_resigns_workspace_idx ON conversations.rule_resigns (workspace_id);

ALTER TABLE conversations.rule_resigns ENABLE ROW LEVEL SECURITY;
ALTER TABLE conversations.rule_resigns FORCE ROW LEVEL SECURITY;
CREATE POLICY workspace_isolation ON conversations.rule_resigns
    USING (workspace_id = nullif(current_setting('campbooks.workspace_id', true), '')::uuid);
-- The dispatcher reads which workspaces still have rows (ids only).
CREATE POLICY maintenance_reads_resigns ON conversations.rule_resigns FOR SELECT
    USING (current_setting('campbooks.maintenance_scope', true) = 'on');

-- The snapshot. Both tables force row-level security even for their owner,
-- and no workspace is set here: it is lifted for this statement only,
-- inside the migration's transaction. The bound fields, as text that no
-- session setting changes (uuids and jsonb print one way; the precedence is
-- printed in UTC by an explicit format). The same expression is in
-- `take_rule_resign` below: keep them identical.
ALTER TABLE conversations.rules NO FORCE ROW LEVEL SECURITY;
ALTER TABLE conversations.rule_resigns NO FORCE ROW LEVEL SECURITY;
INSERT INTO conversations.rule_resigns (rule_id, workspace_id, content_hash)
SELECT r.id, r.workspace_id,
       sha256(convert_to(jsonb_build_array(
           r.workspace_id, r.inbox_id, r.taught_by, r.state, r.conditions, r.actions,
           to_char(coalesce(r.confirmed_at, r.created_at) AT TIME ZONE 'UTC',
                   'YYYY-MM-DD"T"HH24:MI:SS.US'))::text, 'UTF8'))
  FROM conversations.rules r
 WHERE r.state IN ('active', 'paused');
ALTER TABLE conversations.rule_resigns FORCE ROW LEVEL SECURITY;
ALTER TABLE conversations.rules FORCE ROW LEVEL SECURITY;

-- The runtime role reads the snapshot (its workspace's rows; the dispatcher
-- the workspaces) and never writes it.
GRANT SELECT ON conversations.rule_resigns TO campbooks_app;

-- Take rule `which` out of the snapshot (in the transaction's workspace) and
-- say whether it is still as it was when the snapshot was taken: kept
-- (active or paused), in the same workspace, with the same bound fields.
-- False when it was not in the snapshot, or changed since. Removing the row
-- either way means a rule is offered for signing once, never again.
CREATE FUNCTION conversations.take_rule_resign(which uuid)
    RETURNS boolean
    LANGUAGE sql
    SECURITY DEFINER
    SET search_path = pg_catalog, pg_temp
AS $$
    WITH taken AS (
        DELETE FROM conversations.rule_resigns s
         WHERE s.rule_id = which
           AND s.workspace_id
               = nullif(current_setting('campbooks.workspace_id', true), '')::uuid
        RETURNING s.workspace_id, s.content_hash
    )
    SELECT EXISTS (
        SELECT 1
          FROM taken t
          JOIN conversations.rules r ON r.id = which AND r.workspace_id = t.workspace_id
         WHERE r.state IN ('active', 'paused')
           AND sha256(convert_to(jsonb_build_array(
                   r.workspace_id, r.inbox_id, r.taught_by, r.state, r.conditions, r.actions,
                   to_char(coalesce(r.confirmed_at, r.created_at) AT TIME ZONE 'UTC',
                           'YYYY-MM-DD"T"HH24:MI:SS.US'))::text, 'UTF8'))
               = t.content_hash)
$$;

-- What the function's statements need, and nothing else. Row-level security
-- still applies to it (the role does not bypass it): the transaction's
-- workspace only.
GRANT USAGE ON SCHEMA conversations TO campbooks_definer;
GRANT SELECT, DELETE ON conversations.rule_resigns TO campbooks_definer;
GRANT SELECT ON conversations.rules TO campbooks_definer;

-- The hand-over (as in `…080000_kernel__definer_owner_role`): the right to
-- create in the schema is lent for it and taken back in this transaction.
GRANT CREATE ON SCHEMA conversations TO campbooks_definer;
ALTER FUNCTION conversations.take_rule_resign(uuid) OWNER TO campbooks_definer;
REVOKE CREATE ON SCHEMA conversations FROM campbooks_definer;

REVOKE ALL ON FUNCTION conversations.take_rule_resign(uuid) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION conversations.take_rule_resign(uuid) TO campbooks_app;
