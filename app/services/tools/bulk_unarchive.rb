module Tools
  # Reverse of Tools::BulkArchive: move the given messages back to the inbox.
  # Backs the bulk-archive Undo snackbar (EmailMessages::BulkController).
  class BulkUnarchive
    def self.call(args = {})
      # Gate to the acting user's readable accounts before any provider mutation.
      scope = EmailMessage.accessible_to(Current.user)
      scope = scope.where(id: args["email_ids"]) if args["email_ids"].present?

      count = 0
      scope.includes(:email_account).find_each.group_by(&:email_account).each do |account, messages|
        client = account.mail_client
        next unless client.respond_to?(:inbox_folder_id) && client.respond_to?(:move_to_folder)

        inbox_id = client.inbox_folder_id
        next unless inbox_id

        provider_ids = messages.map(&:provider_message_id).compact
        next if provider_ids.empty?

        client.move_to_folder(provider_ids, inbox_id)
        account.email_messages.where(id: messages.map(&:id)).update_all(
          provider_folder_id: inbox_id,
          updated_at: Time.current
        )
        count += messages.size
      rescue => e
        Rails.logger.error("[Tools::BulkUnarchive] Failed for account #{account.id}: #{e.message}")
      end

      { unarchived_count: count }
    end
  end
end
