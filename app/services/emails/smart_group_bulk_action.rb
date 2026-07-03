# frozen_string_literal: true

module Emails
  # Bulk "clear the bucket" actions for a smart group's drill-in view. Resolves
  # the bucket's bundled threads for the acting user, narrows to their inbox
  # copies (the messages the view actually shows), and delegates to the
  # existing security-scoped bulk tools — Tools::BulkArchive/BulkMarkRead both
  # re-gate through EmailMessage.accessible_to(Current.user).
  class SmartGroupBulkAction
    def initialize(user, bucket)
      @user = user
      @bucket = bucket.to_s
    end

    def archive_all
      ids = message_ids
      return 0 if ids.empty?

      Tools::BulkArchive.call("email_ids" => ids)[:archived_count]
    end

    def mark_all_read
      ids = message_ids
      return 0 if ids.empty?

      Tools::BulkMarkRead.call(email_ids: ids, read: true)[:count]
    end

    private

    def message_ids
      accounts = @user.readable_email_accounts.to_a
      smart_groups = Emails::SmartGroups.new(@user, accounts.map(&:id))
      threads = smart_groups.bundled_scope_for(@bucket)
      return [] unless threads

      messages = EmailMessage.where(email_thread_id: threads.select(:id))
      Emails::InboxFolders.constrain(messages, accounts).pluck(:id)
    end
  end
end
