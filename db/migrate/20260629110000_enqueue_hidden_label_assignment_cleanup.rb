class EnqueueHiddenLabelAssignmentCleanup < ActiveRecord::Migration[8.1]
  # Migration 20260629050000 marked provider system/category labels hidden but left
  # their old per-message EmailMessageTag rows behind (noise). Rather than asking
  # operators to run a rake task after upgrading — this is self-hosted, upgrades
  # must be one step — enqueue the idempotent, batched cleanup job so the
  # potentially-large DELETE runs on the worker, off the boot/db:prepare critical
  # path.
  #
  # Best-effort and non-fatal: the chips are already hidden by the `hidden` render
  # gate regardless, and a re-sync also self-heals, so a failed enqueue (e.g. the
  # queue isn't reachable yet during boot) must not break the migration/deploy.
  def up
    Labels::CleanupHiddenAssignmentsJob.perform_later
  rescue StandardError => e
    say "Skipped hidden-label assignment cleanup enqueue (#{e.class}: #{e.message}); a re-sync self-heals."
  end

  def down
    # No-op: the removed rows are reconstructable by a normal label re-sync.
  end
end
