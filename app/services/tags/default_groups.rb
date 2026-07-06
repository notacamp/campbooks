# frozen_string_literal: true

module Tags
  # The four built-in tag groups every workspace ships with — Notifications,
  # Newsletters & promos, Social, and Updates. Each is an ordinary local tag with
  # `group_name` set (so it collapses in the inbox via the general tag-group
  # machinery) plus a `default_bucket` that ties it to a rules-engine category.
  #
  # `default_bucket` is the STABLE identity: the category->tag bridge
  # (EmailProcessJob) and re-provisioning both look a tag up by it, so the user is
  # free to rename, recolor, or move the tag to another group without breaking
  # auto-tagging. Provisioning is idempotent and never clobbers user edits.
  class DefaultGroups
    BUCKETS = %w[notifications promotions social updates].freeze

    # From the tag picker's sanctioned palette (Campbooks::ColorDotSwatches), so
    # each default tag shows as a selected swatch when the user edits it. Red is
    # deliberately avoided — it reads as the error/destructive status color.
    COLORS = {
      "notifications" => "#767988", # slate
      "promotions"    => "#d44996", # pink
      "social"        => "#0584da", # blue
      "updates"       => "#00a8a8"  # teal
    }.freeze

    class << self
      # Ensure the four default group tags exist for the workspace. Idempotent:
      # a tag already tied to a bucket is left untouched (preserving any user
      # rename/recolor/regroup). Returns the four tags.
      def provision!(workspace)
        BUCKETS.map { |bucket| ensure_tag!(workspace, bucket) }
      end

      # The workspace's tag for a rules category, or nil for a non-bucket
      # category (personal/important/unknown) or before provisioning.
      def bucket_tag_for(workspace, bucket)
        bucket = bucket.to_s
        return nil unless BUCKETS.include?(bucket)

        workspace.tags.find_by(default_bucket: bucket)
      end

      # The category->tag bridge: attach the default group tag matching this
      # email's persisted rules category (no-op for personal/important/unknown/
      # nil). Self-heals a workspace still missing its default groups. Idempotent
      # via the composite unique index on email_message_tags. Returns the tag or
      # nil. Callers (EmailProcessJob / EmailRetagJob) wrap this in a rescue so a
      # tagging hiccup never fails the ingest.
      def tag_email!(email)
        bucket = email.category.to_s
        return unless BUCKETS.include?(bucket)

        workspace = email.email_account.workspace
        tag = bucket_tag_for(workspace, bucket)
        if tag.nil?
          provision!(workspace)
          tag = bucket_tag_for(workspace, bucket)
        end
        email.email_message_tags.find_or_create_by!(tag: tag) if tag
        tag
      end

      private

      def ensure_tag!(workspace, bucket)
        existing = workspace.tags.find_by(default_bucket: bucket)
        return existing if existing

        name = display_name(bucket)
        workspace.tags.create!(
          name: name,
          color: COLORS.fetch(bucket),
          group_name: name,
          default_bucket: bucket,
          source: :local,
          kind: :user,
          hidden: false
        )
      rescue ActiveRecord::RecordNotUnique, ActiveRecord::RecordInvalid => e
        # A concurrent provision (partial unique index on workspace+bucket) or a
        # pre-existing local tag with the same name (name is unique per
        # workspace): adopt whatever is already there into this bucket rather
        # than fail. Re-check the bucket first so a race resolves cleanly.
        adopt = workspace.tags.find_by(default_bucket: bucket) ||
                workspace.tags.local.find_by(name: display_name(bucket))
        raise e unless adopt

        adopt.update!(default_bucket: bucket, group_name: adopt.group_name.presence || display_name(bucket))
        adopt
      end

      # Localized at provision time (workspace creator's locale, or the default
      # locale for background self-heal). Snapshotted as the tag name/group_name;
      # the user can rename afterwards.
      def display_name(bucket)
        I18n.t("tag_groups.default_names.#{bucket}")
      end
    end
  end
end
