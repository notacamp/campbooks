# Operational-data retention (GDPR storage limitation, Art. 5(1)(e)). Prunes
# LOGS, workflow run-history, and dismissed feed cards past their window so these
# operational rows don't accumulate forever. Mirrors SessionsPruneJob.
#
# It deliberately does NOT touch user content — emails, documents, contacts,
# calendar events, and AI conversations are never auto-deleted here. Content
# retention (auto-deleting old mail/documents) would be an explicit, per-workspace
# opt-in; it is intentionally not built, so we never silently delete user data.
class RetentionSweepJob < ApplicationJob
  queue_as :default

  LOG_RETENTION = 90.days
  DISMISSED_FEED_RETENTION = 30.days
  AUDIT_EVENT_RETENTION = 12.months

  def perform
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
end
