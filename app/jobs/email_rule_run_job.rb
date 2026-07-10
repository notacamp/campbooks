# frozen_string_literal: true

# Runs a retroactive EmailRuleRun: evaluates the rule's matcher against the
# existing inbox and applies the rule's actions to each matching email.
#
# Safe to retry and to resume: all actions in Applier are idempotent, so a
# retried job re-sweeps from the start and no-ops everything already applied.
# Only terminal states (completed / undone) refuse re-entry — a run stranded in
# `running` by a crashed worker, or marked `failed` by an earlier attempt, is
# picked up again by the Solid Queue retry.
class EmailRuleRunJob < ApplicationJob
  queue_as :default
  retry_on StandardError, wait: :polynomially_longer, attempts: 3

  # Threshold above which undo bookkeeping is skipped (storing 25k+ UUIDs in a
  # jsonb column is wasteful and the undo UI hides the button above this count).
  UNDOABLE_THRESHOLD = 25_000

  def perform(run_id)
    run = EmailRuleRun.find(run_id)
    return if run.completed? || run.undone?

    rule = run.email_rule

    # processed_count restarts on resume (idempotent re-sweep); the undo arrays
    # keep what earlier attempts persisted so no applied change is orphaned.
    run.update!(status: :running, processed_count: 0)

    # Snapshot the matching ids now.  Mail that arrives mid-run is handled by
    # the ingest hook, not this sweep.
    matcher = EmailRules::Matcher.new(rule)
    email_ids = matcher.scope.ids
    matched_count = email_ids.size

    run.update!(
      matched_count: matched_count,
      undoable: run.undoable? && matched_count <= UNDOABLE_THRESHOLD
    )

    # One archive-folder lookup per account for the whole run (Zoho resolves it
    # via a folder-list API call).
    archive_folder_cache = {}

    # Batch in slices of 200 to bound memory.  processed_count is updated per
    # batch (one UPDATE per slice) so the UI can show coarse progress.  A single
    # bad email (e.g. a provider 404 for a since-deleted message) is logged and
    # skipped — it must not strand the whole run.
    email_ids.each_slice(200) do |slice|
      EmailMessage.where(id: slice).includes(:email_account).each do |email|
        EmailRules::Applier.new(email, archive_folder_cache: archive_folder_cache)
          .apply(rule, run: run)
      rescue => e
        Rails.logger.error("[EmailRuleRunJob] run #{run.id}: email #{email.id} failed: #{e.message}")
      end

      # Persist undo arrays incrementally so they survive a mid-run crash.
      run.update_columns(
        processed_count: run.processed_count + slice.size,
        tagged_email_ids:      run.tagged_email_ids,
        archived_email_ids:    run.archived_email_ids,
        marked_read_email_ids: run.marked_read_email_ids,
        moved_email_ids:       run.moved_email_ids,
        updated_at:            Time.current
      )
    end

    now = Time.current
    run.update!(status: :completed, finished_at: now)
    rule.update_columns(last_run_at: now, updated_at: now)
  rescue ActiveRecord::RecordNotFound
    nil
  rescue => e
    run&.update_columns(status: EmailRuleRun.statuses[:failed], updated_at: Time.current)
    raise
  end
end
