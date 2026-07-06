# frozen_string_literal: true

module Tags
  # One-time (idempotent) backfill that attaches the default group "bucket" tags
  # to mail already categorized before the category->tag bridge existed. Without
  # it, only mail processed after the upgrade would collapse into the new default
  # groups; this lets existing inboxes self-heal with no manual step.
  #
  # Batched and conflict-safe (ON CONFLICT DO NOTHING via the composite unique
  # index on email_message_tags), so it never blocks boot and is safe to re-run.
  # Enqueued automatically by a data migration; also runnable directly with
  # `Tags::BackfillDefaultBucketTagsJob.perform_now`.
  class BackfillDefaultBucketTagsJob < ApplicationJob
    queue_as :default

    BATCH_SIZE = 500

    def perform
      Workspace.find_each { |workspace| backfill_workspace(workspace) }
    end

    private

    def backfill_workspace(workspace)
      Tags::DefaultGroups.provision!(workspace)
      bucket_tag_ids = workspace.tags.where.not(default_bucket: nil).pluck(:default_bucket, :id).to_h
      return if bucket_tag_ids.empty?

      Tags::DefaultGroups::BUCKETS.each do |bucket|
        tag_id = bucket_tag_ids[bucket]
        next unless tag_id

        EmailMessage.joins(:email_account)
                    .where(email_accounts: { workspace_id: workspace.id })
                    .where(category: bucket)
                    .in_batches(of: BATCH_SIZE) do |batch|
          now = Time.current
          rows = batch.pluck(:id).map do |message_id|
            { email_message_id: message_id, tag_id: tag_id, created_at: now, updated_at: now }
          end
          EmailMessageTag.insert_all(rows, unique_by: :idx_email_message_tags_unique) if rows.any?
        end
      end
    end
  end
end
