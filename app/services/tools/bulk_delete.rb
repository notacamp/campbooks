module Tools
  class BulkDelete
    def self.call(email_ids:)
      base = EmailMessage.accessible_to(Current.user)
      messages = base.where(id: email_ids)
      thread_ids = messages.where.not(email_thread_id: nil).pluck(:email_thread_id).uniq
      scope = base.where(email_thread_id: thread_ids)

      # Call Zoho trash per account before destroying
      scope.group_by(&:email_account_id).each do |account_id, msgs|
        provider_ids = msgs.map(&:provider_message_id).compact
        next if provider_ids.empty?
        account = EmailAccount.find(account_id)
        client = account.mail_client
        client.trash_messages(provider_ids)
      end

      count = scope.count
      scope.destroy_all
      { count: count }
    end
  end
end
