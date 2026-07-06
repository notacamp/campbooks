# frozen_string_literal: true

module Tags
  # One-time (idempotent) backfill: for every existing external Tag
  # (email_account_id + external_label_id set), create a TagAccountLink and a
  # LabelImportDecision marked "kept" — so the label-review banner never re-surfaces
  # labels the workspace was already using before this feature shipped.
  #
  # Batched and conflict-safe (INSERT OR IGNORE / ON CONFLICT DO NOTHING via the
  # composite unique indexes). Safe to re-run. Enqueued automatically by migration
  # 20260706160100; also runnable directly with perform_now.
  class BackfillAccountLinksJob < ApplicationJob
    queue_as :default

    BATCH_SIZE = 200

    def perform
      Tag.external.where.not(email_account_id: nil, external_label_id: nil)
         .in_batches(of: BATCH_SIZE) do |batch|
        batch.includes(:email_account).each do |tag|
          backfill_tag(tag)
        rescue => e
          Rails.logger.error("[Tags::BackfillAccountLinksJob] tag #{tag.id}: #{e.message}")
        end
      end
    end

    private

    def backfill_tag(tag)
      account = tag.email_account
      return unless account

      # Create the pointer link.
      TagAccountLink.find_or_create_by!(
        tag_id:           tag.id,
        email_account_id: account.id
      ) do |link|
        link.provider_label_id   = tag.external_label_id
        link.provider_label_name = tag.name
      end

      # Record a "kept" decision so review never re-surfaces this label.
      LabelImportDecision.find_or_create_by!(
        email_account_id: account.id,
        provider_label_id: tag.external_label_id
      ) do |dec|
        dec.provider_label_name = tag.name
        dec.decision            = :kept
        dec.tag                 = tag
        dec.reviewed_at         = tag.created_at
      end
    end
  end
end
