module Emails
  module SyncStrategies
    # Microsoft Graph delta sync. Graph's change feed is per-folder, so each
    # email_folders row carries its own deltaLink token. A delta call with no token
    # bootstraps the folder (returns its full current state + a token); thereafter
    # it returns only adds/updates/removes. read/flag changes ride the same feed,
    # so no periodic full walk is needed.
    class Microsoft < Base
      def supports_delta? = true

      def sync!(scan_log: nil)
        up = upserter(scan_log)
        Emails::FolderSync.call(account).reduce(Result.empty) do |acc, folder|
          acc.merge(sync_folder(folder, up))
        end
      end

      # Re-baseline: drop every folder's token so each one bootstraps afresh. Shares
      # the per-folder path — there's no separate full walk to maintain.
      def full_resync!(scan_log: nil)
        up = upserter(scan_log)
        Emails::FolderSync.call(account).reduce(Result.empty) do |acc, folder|
          folder.update_columns(delta_token: nil)
          acc.merge(sync_folder(folder, up))
        end
      end

      private

      def sync_folder(folder, up)
        delta = client.list_messages_delta(folder_id: folder.provider_folder_id, delta_link: folder.delta_token)
        result = delta[:messages].reduce(Result.empty) { |acc, msg| acc.add(up.upsert(msg)) }
        log_removes(folder, delta[:removed_ids])
        folder.update_columns(delta_token: delta[:delta_link]) if delta[:delta_link].present?
        result
      rescue Emails::CursorExpired
        # Token aged out for this folder — drop it; the next pass bootstraps it.
        folder.update_columns(delta_token: nil)
        Result.empty
      end

      # Removes are not applied to our copy yet (mirrors the prior scan, which never
      # deleted) — surfaced so the deferred gap is visible, not silent.
      def log_removes(folder, ids)
        return if ids.blank?

        Rails.logger.info("[Emails::SyncStrategies::Microsoft] #{account.email_address} folder #{folder.provider_folder_id}: #{ids.size} provider removes not applied (deferred)")
      end
    end
  end
end
