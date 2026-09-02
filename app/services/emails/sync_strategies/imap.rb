# frozen_string_literal: true

module Emails
  module SyncStrategies
    # IMAP has no server-push change feed, so it rides the minute poll (#sync!)
    # and 15-minute resync sweep (#full_resync!) like Zoho. The key difference is
    # the UIDVALIDITY/UIDNEXT cursor stored per folder in delta_token: when nothing
    # new has arrived (uidnext unchanged) the strategy issues zero FETCH calls to
    # the client — the common case every minute.
    class Imap < Base
      # Periodic resync of cursored folders is clamped to this window to prevent
      # re-walking 100k-message inboxes on every 15-minute sweep; read-flag drift
      # older than 90 days is not worth that cost.
      RESYNC_HORIZON = 90.days

      def supports_delta? = false

      def sync!(scan_log: nil)
        each_folder_isolated(scan_log) { |folder, up| sync_folder(folder, up) }
      ensure
        client.disconnect
      end

      def full_resync!(scan_log: nil)
        each_folder_isolated(scan_log) { |folder, up| full_resync_folder(folder, up) }
      ensure
        client.disconnect
      end

      private

      # One folder blowing up must not abort the rest — mirrors Zoho's pattern
      # verbatim. Failures land on the scan log so the run reads
      # completed-with-errors instead of losing the other folders' counts.
      def each_folder_isolated(scan_log)
        up     = upserter(scan_log)
        errors = []
        result = Emails::FolderSync.call(account).reduce(Result.empty) do |acc, folder|
          acc.merge(yield(folder, up))
        rescue AuthenticationError
          # A rejected login is account-level, not a bad folder: isolating it
          # would retry LOGIN once per folder per minute forever and starve the
          # engine's PermanentAuthError -> deactivate_for!(:credentials_invalid)
          # path of the signal it exists for. Let it abort the run.
          raise
        rescue => e
          Rails.logger.error("[Emails::SyncStrategies::Imap] #{account.email_address} folder #{folder.name}: #{e.class}: #{e.message}")
          errors << { folder: folder.name, error: "#{e.class}: #{e.message.to_s.first(200)}" }
          acc
        end
        scan_log&.update!(error_messages: errors) if errors.any?
        result
      end

      # Captures folder status BEFORE any walk so mail arriving mid-walk is
      # caught on the next minute poll rather than silently dropped.
      def sync_folder(folder, up)
        fid    = folder.provider_folder_id
        status = client.folder_status(fid)
        cursor = parse_cursor(folder.delta_token)

        if cursor.nil?
          # New folder that appeared mid-life: no cursor yet. Walk the clamped
          # window — not the full mailbox — to bound the catch-up cost.
          msgs   = client.fetch_window(fid, since: clamped_window)
          result = ingest(msgs, up)
          store_cursor(folder, status[:uidvalidity], status[:uidnext])
          result
        else
          stored_validity, stored_uidnext = cursor

          if status[:uidvalidity] != stored_validity
            # The server rebuilt the mailbox; every stored UID is invalid.
            # Re-walk the clamped window — Message-ID identity means re-seen
            # mail reconciles rather than duplicates.
            msgs   = client.fetch_window(fid, since: clamped_window)
            result = ingest(msgs, up)
            store_cursor(folder, status[:uidvalidity], status[:uidnext])
            result
          elsif status[:uidnext] == stored_uidnext
            # Nothing arrived since the last pass — zero FETCH calls. This is
            # the common case on the minute poll.
            Result.empty
          else
            msgs   = client.fetch_new_messages(fid, from_uid: stored_uidnext)
            result = ingest(msgs, up)
            store_cursor(folder, status[:uidvalidity], status[:uidnext])
            result
          end
        end
      end

      def full_resync_folder(folder, up)
        fid    = folder.provider_folder_id
        status = client.folder_status(fid)

        # Cursored folders use the clamped window so a 15-minute sweep does not
        # re-walk years of mail. Cursor-less folders honor the account's chosen
        # backfill depth (nil = whole mailbox for first-connect bootstrap).
        since = if folder.delta_token.present?
          clamped_window
        else
          account.backfill_since&.to_date
        end

        msgs   = client.fetch_window(fid, since: since)
        result = ingest(msgs, up)
        store_cursor(folder, status[:uidvalidity], status[:uidnext])
        result
      end

      def ingest(msgs, up)
        Array(msgs).reduce(Result.empty) { |acc, msg| acc.add(up.upsert(msg)) }
      end

      # Rolling window that constrains sync! and full_resync! cursored folder walks.
      # Takes the more-recent of the account's backfill choice and 90 days ago so
      # that a user who chose "last 30 days" is still respected, while one who chose
      # "all time" is clamped to 90 days on the periodic pass.
      def clamped_window
        [ account.backfill_since, RESYNC_HORIZON.ago ].compact.max&.to_date
      end

      # Cursor format: "<uidvalidity>:<uidnext>" stored in email_folders.delta_token.
      # Returns [validity, uidnext] or nil if the token is absent or unparseable.
      def parse_cursor(token)
        return nil unless token.present?

        parts = token.to_s.split(":")
        return nil unless parts.size == 2

        validity = parts[0].to_i
        uidnext  = parts[1].to_i
        return nil if validity.zero? || uidnext.zero?

        [ validity, uidnext ]
      end

      def store_cursor(folder, validity, uidnext)
        folder.update_columns(delta_token: "#{validity}:#{uidnext}", last_synced_at: Time.current)
      end
    end
  end
end
