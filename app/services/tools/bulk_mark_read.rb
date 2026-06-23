module Tools
  class BulkMarkRead
    def self.call(email_ids:, read:)
      base = EmailMessage.accessible_to(Current.user)
      messages = base.where(id: email_ids)
      thread_ids = messages.where.not(email_thread_id: nil).pluck(:email_thread_id).uniq
      scope = base.where(email_thread_id: thread_ids)

      scope.update_all(read: read, updated_at: Time.current)

      # Call Zoho API per account
      scope.group_by(&:email_account_id).each do |account_id, msgs|
        provider_ids = msgs.map(&:provider_message_id).compact
        next if provider_ids.empty?
        if read
          MarkReadJob.perform_later(account_id, provider_ids)
        else
          MarkUnreadJob.perform_later(account_id, provider_ids)
        end
      end

      { count: scope.count, read: read }
    end
  end
end
