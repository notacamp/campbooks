# frozen_string_literal: true

module Emails
  # Reverses a Skim-Mode archive: moves a set of emails back into their account's
  # Inbox folder (the "Undo" on the archive toast). Security-scoped exactly like
  # Emails::SkimArchive — only mail the user can read is touched, so a forged id
  # list can't reach another account's inbox. Mirrors Tools::BulkArchive's
  # provider move, in the opposite direction.
  class SkimRestore
    def initialize(user, raw_ids)
      @user = user
      @ids = Emails::SkimArchive.sanitize_ids(raw_ids)
    end

    # Returns the number of emails actually moved back to the inbox.
    def call
      return 0 if @ids.empty?

      owned = EmailMessage.where(id: @ids, email_account: @user.readable_email_accounts)
      restored = 0

      owned.includes(:email_account).group_by(&:email_account).each do |account, messages|
        client = account.mail_client
        next unless client.respond_to?(:inbox_folder_id)

        folder_id = (client.inbox_folder_id rescue nil)
        next unless folder_id

        provider_ids = messages.map(&:provider_message_id).compact
        next if provider_ids.empty?

        client.move_to_folder(provider_ids, folder_id)
        account.email_messages.where(id: messages.map(&:id)).update_all(
          provider_folder_id: folder_id,
          updated_at: Time.current
        )
        restored += messages.size
      rescue => e
        Rails.logger.error("[Emails::SkimRestore] Failed for account #{account.id}: #{e.message}")
      end

      restored
    end
  end
end
