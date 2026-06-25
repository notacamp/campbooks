module MailFolders
  # Creates (or locates) the real provider-side folder for a custom MailFolder on
  # each connected account, idempotently. Gmail has no folders, so on Google the
  # folder is created as a *label* (which Gmail's list_folders surfaces and
  # move_to_folder targets) — the rest of the app treats it as a folder by name.
  #
  # Reused by MailFoldersController (provision on create) and Tools::BulkMoveToFolder
  # (lazy-ensure the destination exists before a by-name move), so a drag right
  # after creating a folder always resolves to a real destination id.
  class Provisioner
    class << self
      # Idempotently returns the provider_folder_id for `name` on `account`,
      # creating the folder/label on the provider if it doesn't exist yet.
      def ensure_on_account(account, name)
        name = name.to_s.strip
        raise ArgumentError, "folder name required" if name.blank?

        # 1. Persisted mirror — instant, the source of truth after any sync/create.
        if (row = account.email_folders.find_by("LOWER(name) = ?", name.downcase))
          return row.provider_folder_id
        end

        # 2. Cached live list, in case it exists on the provider but isn't mirrored.
        if (live = account.folders.find { |f| f[:name].to_s.casecmp?(name) })
          return persist_folder(account, live[:id], name)
        end

        # 3. Create on the provider; if that yields no id (e.g. it already exists),
        #    re-read the list bypassing the cache and match by name.
        provider_id = create_remote_folder(account, name) || fresh_lookup(account, name)
        unless provider_id.present?
          raise "Could not create or find folder #{name.inspect} on #{account.email_address}"
        end

        persist_folder(account, provider_id, name)
      end

      # Provision `mail_folder` across every active account in its workspace that
      # `user` can manage. Returns { created: [accounts], failed: [accounts] };
      # a single account's failure never aborts the others.
      def provision_all(mail_folder, user)
        created = []
        failed = []

        mail_folder.workspace.email_accounts.active.each do |account|
          next unless account.managed_by?(user)

          begin
            ensure_on_account(account, mail_folder.name)
            created << account
          rescue => e
            Rails.logger.error("[MailFolders::Provisioner] #{mail_folder.name.inspect} failed on #{account.email_address}: #{e.message}")
            failed << account
          end
        end

        { created: created, failed: failed }
      end

      # Rename `mail_folder`'s provider folder on every active account in its
      # workspace that `user` can manage, locating each provider folder by its OLD
      # name in the mirror. Returns { renamed: [accounts], failed: [accounts] };
      # one account's failure never aborts the others, and accounts without a
      # mirror row for the old name are simply skipped.
      def rename_all(mail_folder, old_name, user)
        renamed = []
        failed = []

        mail_folder.workspace.email_accounts.active.each do |account|
          next unless account.managed_by?(user)

          mirror = account.email_folders.find_by("LOWER(name) = ?", old_name.to_s.downcase)
          next unless mirror

          begin
            rename_remote_folder(account, mirror.provider_folder_id, mail_folder.name)
            mirror.update!(name: mail_folder.name)
            Rails.cache.delete("email_account/#{account.id}/folders")
            renamed << account
          rescue => e
            Rails.logger.error("[MailFolders::Provisioner] rename #{old_name.inspect} → #{mail_folder.name.inspect} failed on #{account.email_address}: #{e.message}")
            failed << account
          end
        end

        { renamed: renamed, failed: failed }
      end

      private

      def create_remote_folder(account, name)
        client = account.mail_client
        case account.provider.to_sym
        when :google
          # Gmail folders are labels.
          client.create_label(name: name)&.dig("id")
        when :microsoft
          client.create_folder(name)&.dig("id")
        else # zoho
          client.create_folder(name)&.dig("folderId")
        end
      end

      def rename_remote_folder(account, provider_folder_id, name)
        client = account.mail_client
        case account.provider.to_sym
        when :google
          client.update_label(provider_folder_id, name: name) # Gmail folders are labels
        else # microsoft, zoho
          client.update_folder(provider_folder_id, name)
        end
      end

      # Re-read the provider's folder list (uncached) and find `name`.
      def fresh_lookup(account, name)
        list = account.mail_client.list_folders rescue []
        list.find { |f| f["folderName"].to_s.casecmp?(name) }&.dig("folderId")
      end

      # Upsert the local email_folders mirror so name-based filtering resolves the
      # new folder immediately, and bust the per-account folder cache.
      def persist_folder(account, provider_id, name)
        row = account.email_folders.find_or_initialize_by(provider_folder_id: provider_id)
        row.name = name
        row.position ||= EmailFolder.default_position_for(name)
        row.save! if row.changed?
        Rails.cache.delete("email_account/#{account.id}/folders")
        provider_id
      end
    end
  end
end
