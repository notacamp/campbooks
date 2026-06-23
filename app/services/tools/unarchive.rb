module Tools
  # Reverse of Tools::Archive: move a thread's messages back to the inbox. Used by
  # the feed's Undo affordance so a mis-tapped Archive is recoverable.
  class Unarchive
    def self.call(email_message, _args = {})
      thread = email_message.email_thread
      return unless thread

      client = email_message.email_account.mail_client
      return unless client.respond_to?(:inbox_folder_id) && client.respond_to?(:move_to_folder)

      inbox_id = client.inbox_folder_id
      return unless inbox_id

      provider_ids = thread.email_messages.pluck(:provider_message_id).compact
      return thread if provider_ids.empty?

      client.move_to_folder(provider_ids, inbox_id)
      thread.email_messages.update_all(provider_folder_id: inbox_id, updated_at: Time.current)
      thread
    rescue => e
      Rails.logger.error("[Tools::Unarchive] Failed for thread #{thread&.id}: #{e.message}")
      nil
    end
  end
end
