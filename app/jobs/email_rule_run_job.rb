# frozen_string_literal: true

# Runs a retroactive EmailRuleRun: evaluates the rule's matcher against the
# existing inbox and applies the rule's actions to each matching email.
#
# Safe to retry (all actions in Applier are idempotent).  The run record is
# checked for a non-queued status on entry so a concurrent retry that catches
# up after a partial run doesn't restart from the beginning.
class EmailRuleRunJob < ApplicationJob
  queue_as :default
  retry_on StandardError, wait: :polynomially_longer, attempts: 3

  # Threshold above which undo bookkeeping is skipped (storing 25k+ UUIDs in a
  # jsonb column is wasteful and the undo UI hides the button above this count).
  UNDOABLE_THRESHOLD = 25_000

  def perform(run_id)
    run = EmailRuleRun.find(run_id)
    return if run.running? || run.completed? || run.undone? || run.failed?

    rule = run.email_rule

    run.update!(status: :running)

    # Snapshot the matching email ids now.  Using ids (an Array) rather than
    # keeping the scope open keeps the batch idempotent on retry: a retried job
    # re-uses the same ids regardless of mail that arrived in the interim.
    matcher = EmailRules::Matcher.new(rule)
    email_ids = matcher.scope.ids
    matched_count = email_ids.size
    undoable = matched_count <= UNDOABLE_THRESHOLD

    run.update!(
      matched_count: matched_count,
      undoable: undoable
    )

    # Batch over ids in slices of 200 to avoid loading the whole set into memory
    # at once.  processed_count is updated per batch (one UPDATE per slice) so
    # the UI can show coarse progress.
    email_ids.each_slice(200) do |slice|
      EmailMessage.where(id: slice).find_each do |email|
        EmailRules::Applier.new(email).apply(rule, run: run)
      end

      # Persist undo arrays incrementally so they survive a mid-run crash.
      run.update_columns(
        processed_count: run.processed_count + slice.size,
        archived_email_ids:    run.archived_email_ids,
        marked_read_email_ids: run.marked_read_email_ids,
        moved_email_ids:       run.moved_email_ids,
        updated_at:            Time.current
      )
    end

    now = Time.current
    run.update!(status: :completed, finished_at: now)
    rule.update_columns(last_run_at: now, updated_at: now)
  rescue => e
    run&.update_columns(status: EmailRuleRun.statuses[:failed], updated_at: Time.current)
    raise
  end
end
