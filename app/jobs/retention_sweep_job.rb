# Operational-data retention (GDPR storage limitation, Art. 5(1)(e)). Prunes
# LOGS, workflow run-history, and dismissed feed cards past their window so these
# operational rows don't accumulate forever. Mirrors SessionsPruneJob.
#
# It does NOT auto-delete user content by default — emails, documents, contacts,
# calendar events, and AI conversations are kept unless a workspace has EXPLICITLY
# opted into content retention (Settings → Data & Privacy → "Auto-delete email").
# The operational-log sweep and the opt-in content sweep are kept in separate,
# clearly-labelled methods so the "never silently delete user data" boundary stays
# auditable.
class RetentionSweepJob < ApplicationJob
  queue_as :default

  LOG_RETENTION = 90.days
  DISMISSED_FEED_RETENTION = 30.days
  AUDIT_EVENT_RETENTION = 12.months

  def perform
    prune_operational_logs
    prune_opted_in_content
  end

  private

  # Operational rows (logs, run-history, dismissed cards, audit events) — never user content.
  def prune_operational_logs
    log_cutoff = LOG_RETENTION.ago

    EmailScanLog.where(created_at: ..log_cutoff).in_batches.delete_all
    CalendarSyncLog.where(created_at: ..log_cutoff).in_batches.delete_all

    # Domain events are operational tracking rows (their payloads can hold email
    # metadata); prune them on the same window as the other logs. The activity
    # feed only ever shows the recent window.
    Event.where(occurred_at: ..log_cutoff).in_batches.delete_all

    # Workflow run-history (trigger/input/output payloads can hold email metadata).
    WorkflowExecution.where(created_at: ..log_cutoff).in_batches do |batch|
      WorkflowExecutionStep.where(workflow_execution_id: batch.ids).delete_all
      batch.delete_all
    end

    # Dismissed home-feed cards are ephemeral UI rows once the user has dismissed them.
    FeedItem.where.not(dismissed_at: nil)
            .where(dismissed_at: ..DISMISSED_FEED_RETENTION.ago)
            .in_batches.delete_all

    # Security/audit events (sign-ins, MFA changes, exports…) are kept long enough
    # to be useful in the Settings → Security log, then pruned so the table doesn't
    # grow forever. Deleted-user rows (user_id NULL) age out the same way.
    AuditEvent.where(created_at: ..AUDIT_EVENT_RETENTION.ago).in_batches.delete_all
  end

  # Opt-in content retention: for each workspace that set email_retention_months,
  # delete email older than that window. This removes ONLY Campbooks' stored copy —
  # the local EmailMessage row plus its search index, tags, and cached attachments.
  # It NEVER contacts the mail provider, so the message stays untouched in the
  # user's actual mailbox (EmailMessage has no provider-side destroy callback; we
  # only ever delete locally). NULL retention = keep forever (the default).
  def prune_opted_in_content
    Workspace.where.not(email_retention_months: nil).find_each do |workspace|
      months = workspace.email_retention_months.to_i
      next unless months.positive?

      account_ids = workspace.email_accounts.ids
      next if account_ids.empty?

      cutoff = months.months.ago
      # received_at IS NULL (unknown age) is excluded by the range, so we never
      # delete mail we can't date.
      scope = EmailMessage.where(email_account_id: account_ids, received_at: ..cutoff)

      deleted = 0
      scope.in_batches do |batch|
        ids = batch.ids
        # search_chunks / search_records are polymorphic and not dependent-destroyed
        # by EmailMessage, so clear our derived index for these rows explicitly.
        SearchChunk.where(searchable_type: "EmailMessage", searchable_id: ids).delete_all
        SearchRecord.where(searchable_type: "EmailMessage", searchable_id: ids).delete_all
        # destroy_all (not delete_all) so dependent local rows + attachment blobs are
        # cleaned up; still purely local — no provider call.
        deleted += batch.destroy_all.size
      end

      log_retention(workspace, months, deleted) if deleted.positive?
    end
  end

  def log_retention(workspace, months, count)
    Rails.logger.info(
      "[RetentionSweepJob] workspace #{workspace.id}: deleted #{count} local email copies " \
      "older than #{months} months (provider mailbox untouched)"
    )
  end
end
