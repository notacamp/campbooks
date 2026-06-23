module Emails
  module SyncStrategies
    # Zoho has no change feed, so it can't do true delta — but that doesn't hold the
    # other vendors back, it just means Zoho's strategy is different. The every-
    # minute #sync! windows *new* mail across every folder with a per-folder
    # received-time watermark (a single newest-first page per folder, cheap), and
    # read/flag changes on existing mail ride the periodic #full_resync! walk
    # (needs_periodic_resync? is true). That replaces the old global 15-minute
    # full re-walk with one that only Zoho accounts pay.
    class Zoho < Base
      PAGE = 200

      def supports_delta? = false

      def sync!(scan_log: nil)
        up = upserter(scan_log)
        Emails::FolderSync.call(account).reduce(Result.empty) do |acc, folder|
          acc.merge(window_new_mail(folder, up))
        end
      end

      def full_resync!(scan_log: nil)
        up = upserter(scan_log)
        Emails::FolderSync.call(account).reduce(Result.empty) do |acc, folder|
          acc.merge(walk_folder(folder, up))
        end
      end

      private

      # One newest-first page per folder; ingest only messages newer than the
      # watermark (older ones are already stored), then advance it. No change feed,
      # but bounded to a single API call per folder when there's no new mail.
      def window_new_mail(folder, up)
        watermark = folder.last_synced_at
        newest = watermark
        result = Result.empty

        client.list_messages(folder_id: folder.provider_folder_id, limit: PAGE, start: 0, skip_known: false).each do |msg|
          received = parse_received(msg["receivedTime"])
          next if watermark && received && received <= watermark

          result = result.add(up.upsert(msg))
          newest = received if received && (newest.nil? || received > newest)
        end

        folder.update_columns(last_synced_at: newest) if newest && newest != watermark
        result
      end

      # Full per-folder walk: creates new mail and reconciles read/flag on existing
      # mail (the drift the windowed pass can't see). The heavy, periodic path.
      def walk_folder(folder, up)
        result = Result.empty
        newest = folder.last_synced_at
        start = 0

        loop do
          msgs = client.list_messages(folder_id: folder.provider_folder_id, limit: PAGE, start: start, skip_known: false)
          break if msgs.empty?

          msgs.each do |msg|
            result = result.add(up.upsert(msg))
            received = parse_received(msg["receivedTime"])
            newest = received if received && (newest.nil? || received > newest)
          end
          break if msgs.size < PAGE

          start += PAGE
        end

        folder.update_columns(last_synced_at: newest) if newest
        result
      end

      def parse_received(received_time)
        return nil if received_time.blank?

        Time.at(received_time.to_i / 1000)
      end
    end
  end
end
