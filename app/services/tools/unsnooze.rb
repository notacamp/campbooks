module Tools
  class Unsnooze
    def self.call(email_message, _args = {})
      thread = email_message.email_thread
      return unless thread
      return thread if thread.snoozed_until.nil?

      account = email_message.email_account
      client = account.mail_client

      provider_ids = thread.email_messages.pluck(:provider_message_id).compact

      if client.respond_to?(:unsnooze_messages) && provider_ids.any?
        client.unsnooze_messages(provider_ids)
      elsif client.respond_to?(:inbox_folder_id) && client.respond_to?(:move_to_folder) && provider_ids.any?
        inbox_id = client.inbox_folder_id
        client.move_to_folder(provider_ids, inbox_id) if inbox_id
      end

      inbox_id = client.respond_to?(:inbox_folder_id) ? client.inbox_folder_id : nil
      if inbox_id
        thread.email_messages.update_all(
          provider_folder_id: inbox_id,
          updated_at: Time.current
        )
      end

      thread.update!(snoozed_until: nil)
      thread
    rescue => e
      Rails.logger.error("[Tools::Unsnooze] Failed for thread #{thread&.id}: #{e.message}")
      nil
    end
  end
end
