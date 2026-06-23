module Tools
  class Trash
    def self.call(email_message, _args = {})
      thread = email_message.email_thread
      return unless thread

      account = email_message.email_account
      client = account.mail_client
      return unless client.respond_to?(:trash_messages)

      provider_ids = thread.email_messages.pluck(:provider_message_id).compact
      return thread if provider_ids.empty?

      client.trash_messages(provider_ids)

      # Find trash folder ID for local sync
      folders = client.list_folders
      trash = folders.find { |f| f["folderName"] == "Trash" }
      if trash
        thread.email_messages.update_all(
          provider_folder_id: trash["folderId"],
          updated_at: Time.current
        )
      end
      Events.publish("email.trashed", subject: email_message, workspace: email_message.email_account.workspace, payload: { "subject" => email_message.subject, "from" => email_message.from_address })
      thread
    rescue => e
      Rails.logger.error("[Tools::Trash] Failed for thread #{thread.id}: #{e.message}")
      nil
    end
  end
end
