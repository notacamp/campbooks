-- Retention of the event log and of finished jobs (campbooks-cloud#117,
-- #65). `kernel.events` is no longer kept forever: `kernel.prune_events`
-- deletes events older than CAMPBOOKS_EVENTS__RETENTION_DAYS unless a
-- delivery is still pending, and a purged workspace loses its events and
-- finished jobs. These indexes keep both cheap.

-- Pruning by age.
CREATE INDEX events_occurred_at_idx ON kernel.events (occurred_at);
-- Purging one workspace's events.
CREATE INDEX events_workspace_idx ON kernel.events (workspace_id)
    WHERE workspace_id IS NOT NULL;
-- "Is this event still waiting for a consumer?"
CREATE INDEX jobs_event_unfinished_idx ON kernel.jobs (event_id)
    WHERE event_id IS NOT NULL AND state IN ('pending', 'running');
