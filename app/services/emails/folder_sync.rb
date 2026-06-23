module Emails
  # Mirrors the provider's folder list into email_folders, so the per-folder sync
  # cursors (Microsoft delta tokens, Zoho received-time watermarks) have a stable
  # home and the UI has a persisted folder list. Idempotent upsert keyed by
  # provider_folder_id; never deletes (a folder vanishing from the provider list
  # shouldn't orphan the messages still pointing at it).
  class FolderSync
    def self.call(account) = new(account).call

    def initialize(account)
      @account = account
    end

    # Returns the account's EmailFolder records (refreshed). Folder listing is a
    # best-effort side concern of a sync run, so a provider hiccup returns whatever
    # is already persisted rather than blowing up the scan.
    def call
      @account.mail_client.list_folders.each do |f|
        fid  = f["folderId"]
        name = f["folderName"].presence || f["name"]
        next if fid.blank? || name.blank?

        folder = @account.email_folders.find_or_initialize_by(provider_folder_id: fid)
        folder.name = name
        folder.position = EmailFolder.default_position_for(name) if folder.new_record?
        folder.save! if folder.changed?
      end
      @account.email_folders.reload.to_a
    rescue => e
      Rails.logger.error("[Emails::FolderSync] #{@account.email_address}: #{e.message}")
      @account.email_folders.to_a
    end
  end
end
