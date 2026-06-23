# frozen_string_literal: true

module Emails
  # Resolves the provider folder id that represents "Archive" for each of a
  # user's accounts, so the Board's Done column can list archived threads. Mirror
  # of Emails::InboxFolders, but the archive folder is a single id per account
  # (client.archive_folder_id) rather than a name scan. Cached briefly since it
  # can hit the mail client's folder listing.
  class ArchiveFolders
    def self.ids_for(accounts)
      accounts = Array(accounts)
      return [] if accounts.empty?

      Rails.cache.fetch("board/archive_folder_ids/#{accounts.map(&:id).sort.join('_')}", expires_in: 5.minutes) do
        accounts.filter_map { |account| archive_id(account) }
      end
    end

    def self.archive_id(account)
      client = account.mail_client
      return nil unless client.respond_to?(:archive_folder_id)

      client.archive_folder_id
    rescue => e
      Rails.logger.warn("[Emails::ArchiveFolders] folder lookup failed for account #{account.id}: #{e.message}")
      nil
    end
  end
end
