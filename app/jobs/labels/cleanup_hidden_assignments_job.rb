module Labels
  # One-shot (re-runnable) cleanup that drops EmailMessageTag rows belonging to
  # hidden labels — provider system/low-value labels that should never have been
  # attached to individual messages. Join-rows only: the labels themselves are
  # kept (the decision is remembered) and the assignments are reconstructable by a
  # re-sync, so this never touches user content.
  class CleanupHiddenAssignmentsJob < ApplicationJob
    queue_as :default

    def perform
      deleted = 0
      EmailMessageTag.where(tag_id: Tag.hidden_labels.select(:id)).in_batches(of: 1000) do |batch|
        deleted += batch.delete_all
      end
      Rails.logger.info("[Labels::CleanupHiddenAssignmentsJob] Removed #{deleted} hidden-label assignments")
      deleted
    end
  end
end
