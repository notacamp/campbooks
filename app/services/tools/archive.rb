module Tools
  class Archive
    def self.call(email_message, _args = {})
      thread = email_message.email_thread
      return unless thread

      account = email_message.email_account
      client = account.mail_client
      return unless client.respond_to?(:archive_folder_id)
      folder_id = client.archive_folder_id
      return unless folder_id

      provider_ids = thread.email_messages.pluck(:provider_message_id).compact
      return thread if provider_ids.empty?

      client.move_to_folder(provider_ids, folder_id)
      thread.email_messages.update_all(
        provider_folder_id: folder_id,
        updated_at: Time.current
      )
      Events.publish("email.archived", subject: email_message, workspace: email_message.email_account.workspace, payload: { "subject" => email_message.subject, "from" => email_message.from_address })
      thread
    rescue => e
      Rails.logger.error("[Tools::Archive] Failed for thread #{thread.id}: #{e.message}")
      nil
    end
  end
end
