-- Rule runs and the changes they record, bound with a MAC under a key the
-- database never holds (campbooks-cloud#242).
--
-- Any transaction working in a workspace can insert a run of a taught rule
-- (and the job that works through it), or a `rule_applications` row whose
-- `before` says what to put back: an undo would then take mail out of spam
-- or the trash at the provider, and a planted apply run would act on old
-- mail no member asked about. A run now carries a MAC (`rule_runs.mac`)
-- over the workspace, the run, its rule, the rule's binding generation, the
-- kind, the rule's inbox and the member who asked for it
-- (`rule_runs.requested_by`), signed only in that member's request; an
-- application carries one (`rule_applications.mac`) over the workspace, the
-- rule, its inbox, the conversation, the run and what it recorded, signed
-- only where the rule's verified effects are recorded. A NULL MAC never
-- verifies.
--
-- Rows written before this have none, and none are signed later: a run
-- still going stops at its next batch (the member can start it again), and
-- what a rule changed before this cannot be put back (removing the rule
-- with `restore=true` answers 409 `undo_unavailable`; removing it keeping
-- its changes still works). See the change's design.md.
SET LOCAL lock_timeout = '10s';

-- Catalog-only: nullable columns, no default.
ALTER TABLE conversations.rule_runs
    ADD COLUMN requested_by uuid,
    ADD COLUMN mac bytea;

ALTER TABLE conversations.rule_applications
    ADD COLUMN mac bytea;
