# frozen_string_literal: true

module Emails
  # Resolves the provider folder ids that represent the "Inbox" across a user's
  # accounts, so Skim shows only inbox mail. This also fixes archived mail
  # reappearing: Tools::BulkArchive moves a message to the Archive folder and
  # updates its provider_folder_id, so an inbox-only filter excludes it. Cached
  # briefly since it hits the mail client's folder listing.
  class InboxFolders
    # Constrain an EmailMessage scope to the inbox folder(s) of the given accounts.
    # Shared by Emails::SkimScope and Feed::Source so Skim and the home feed agree
    # on "still in the inbox" — archiving moves a message to the Archive folder
    # (rewriting provider_folder_id), so this drops archived mail from both.
    # Fails open (no filter) when folder ids can't be resolved, so a transient
    # mail-client hiccup never empties the inbox view.
    def self.constrain(scope, accounts)
      ids = ids_for(accounts)
      ids.any? ? scope.where(provider_folder_id: ids) : scope
    end

    def self.ids_for(accounts)
      accounts = Array(accounts)
      return [] if accounts.empty?

      cache_key = "skim/inbox_folder_ids/#{accounts.map(&:id).sort.join('_')}"
      cached = Rails.cache.read(cache_key)
      return cached if cached

      # An empty result is indistinguishable from a transient provider failure
      # (the mail client returned nothing or errored). Caching empty for 5 minutes
      # would silence live-inbox broadcasts and skim filters for that entire window.
      # Only write the cache when the result is non-empty so a transient failure
      # retries on the very next call rather than pinning 5 minutes of blindness.
      result = accounts.flat_map { |account| inbox_ids(account) }.compact
      Rails.cache.write(cache_key, result, expires_in: 5.minutes) if result.any?
      result
    end

    def self.inbox_ids(account)
      client = account.mail_client
      return [] unless client

      (client.list_folders || []).filter_map do |folder|
        folder["folderId"] if folder["folderName"].to_s.strip.casecmp?("inbox")
      end
    rescue => e
      Rails.logger.warn("[Emails::InboxFolders] folder lookup failed for account #{account.id}: #{e.message}")
      []
    end
  end
end
