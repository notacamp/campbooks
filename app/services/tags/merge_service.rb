# frozen_string_literal: true

module Tags
  # Merges tag B (the source) into tag A (the target): all of B's email message
  # associations and account-pointer links move to A, then B is destroyed.
  #
  # Guards:
  #   - Both tags must belong to the same workspace (cross-workspace merge raises).
  #   - A tag cannot be merged into itself.
  #   - Never touches provider-side data — the merge is purely local.
  #
  # Deduplication:
  #   - EmailMessageTag: INSERT ON CONFLICT DO NOTHING via insert_all.
  #   - TagAccountLink: same.
  #   - LabelImportDecision: linked decisions are re-pointed to A.
  #
  # default_bucket preservation: if A has no default_bucket and B does, A adopts
  # B's bucket (keeps the auto-tagging bridge key intact). If both have one,
  # A's bucket wins. group_name is never touched — merge does not affect grouping.
  class MergeService
    class MergeError < StandardError; end

    def initialize(source:, target:)
      @source = source
      @target = target
    end

    # Returns the surviving tag (target) on success, raises MergeError on guard failure.
    def merge!
      validate!

      ActiveRecord::Base.transaction do
        adopt_default_bucket!
        move_message_tags!
        move_account_links!
        repoint_import_decisions!
        @source.destroy!
      end

      @target
    end

    private

    def validate!
      raise MergeError, "cannot merge a tag into itself" if @source.id == @target.id

      if @source.workspace_id != @target.workspace_id
        raise MergeError, "cannot merge tags across workspaces"
      end
    end

    # If the target has no default_bucket, inherit it from the source. We must
    # clear the source's bucket first inside the transaction to satisfy the
    # unique partial index (workspace_id, default_bucket) before setting it on
    # the target. The source will be destroyed at the end of the same transaction.
    #
    # NOTE: group_name is intentionally NOT touched here. Merging is about
    # consolidating tag identity (message links + account pointers + the
    # auto-tagging bucket key). Grouping is a separate, user-controlled
    # classification axis. If the surviving target ends up bucketed but ungrouped,
    # that is correct — the user can add it to a group themselves.
    def adopt_default_bucket!
      return if @target.default_bucket.present?
      return if @source.default_bucket.blank?

      bucket = @source.default_bucket
      @source.update_columns(default_bucket: nil)
      @target.update_columns(default_bucket: bucket)
    end

    # Move all EmailMessageTag rows from source to target, skipping duplicates
    # (messages already tagged with the target tag).
    def move_message_tags!
      now  = Time.current
      rows = EmailMessageTag
             .where(tag_id: @source.id)
             .pluck(:email_message_id)
             .map { |mid| { email_message_id: mid, tag_id: @target.id, created_at: now, updated_at: now } }

      EmailMessageTag.insert_all(rows, unique_by: :idx_email_message_tags_unique) if rows.any?

      # Remove the source rows (insert_all already wrote the surviving links).
      EmailMessageTag.where(tag_id: @source.id).delete_all
    end

    # Move TagAccountLink rows from source to target, skipping collisions
    # (target already linked to the same account/label).
    # We delete the source links first to avoid PG unique-constraint violations
    # on (email_account_id, provider_label_id) when inserting for target.
    def move_account_links!
      now  = Time.current
      rows = TagAccountLink
             .where(tag_id: @source.id)
             .pluck(:email_account_id, :provider_label_id, :provider_label_name)
             .map { |aid, lid, lname|
               { tag_id: @target.id, email_account_id: aid,
                 provider_label_id: lid, provider_label_name: lname,
                 created_at: now, updated_at: now }
             }

      # Delete source links before inserting for target so the unique index on
      # (email_account_id, provider_label_id) never fires.
      TagAccountLink.where(tag_id: @source.id).delete_all

      TagAccountLink.insert_all(rows, unique_by: :idx_tag_account_links_tag_account) if rows.any?
    end

    # Re-point any LabelImportDecision that currently references the source tag.
    def repoint_import_decisions!
      LabelImportDecision.where(tag_id: @source.id)
                         .update_all(tag_id: @target.id)
    end
  end
end
