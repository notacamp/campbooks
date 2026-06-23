module Emails
  module SyncStrategies
    # Gmail delta sync via users.history.list. Gmail's history is mailbox-wide —
    # one stored historyId (on the account) tracks adds, deletes and read/label
    # changes across every label — so the every-minute pull is a single cheap call
    # and the full folder walk only runs to bootstrap or recover the cursor.
    class Google < Base
      PAGE = 200

      def supports_delta? = true

      # Gmail's cursor is the account-wide historyId, not the folder list.
      def needs_bootstrap? = account.history_id.blank?

      def sync!(scan_log: nil)
        # A missing cursor is treated like an expired one: signal the engine to
        # offload the heavy re-baseline to Emails::FullResyncJob rather than walk
        # the whole mailbox inline in the minute poll.
        raise Emails::CursorExpired, "no Gmail historyId for #{account.email_address}" if account.history_id.blank?

        delta = client.list_history(start_history_id: account.history_id)
        result = ingest(delta[:changed_ids], scan_log)
        log_deletes(delta[:deleted_ids])
        account.update_columns(history_id: delta[:history_id]) if delta[:history_id].present?
        result
      end

      def full_resync!(scan_log: nil)
        folders = Emails::FolderSync.call(account)
        # Baseline the cursor from "now" BEFORE the walk, so mail that lands mid-walk
        # is caught by the next incremental pull instead of slipping through the gap.
        baseline = client.current_history_id
        result = walk(folders.map(&:provider_folder_id), scan_log)
        account.update_columns(history_id: baseline) if baseline.present?
        result
      end

      private

      def ingest(ids, scan_log)
        up = upserter(scan_log)
        client.fetch_messages(ids).reduce(Result.empty) { |acc, msg| acc.add(up.upsert(msg)) }
      end

      def walk(folder_ids, scan_log)
        up = upserter(scan_log)
        result = Result.empty
        folder_ids.each do |fid|
          start = 0
          loop do
            # skip_known: the bootstrap only needs to ingest mail we don't already
            # have and establish the cursor. Re-fetching metadata for every stored
            # message (a GET each) is what made the old full sweep crawl — and on a
            # 10k mailbox, wedge the worker. read/flag drift on existing mail rides
            # the history delta, not this walk.
            msgs = client.list_messages(folder_id: fid, limit: PAGE, start: start, skip_known: true)
            # Newest-first: an all-known (post-filter empty) page means everything
            # older is known too, so there's nothing left to ingest in this folder.
            break if msgs.empty?

            msgs.each { |m| result = result.add(up.upsert(m)) }
            break unless client.more_messages? # page token, not msgs.size — skip_known shrinks the page

            start += PAGE
          end
        end
        result
      end

      # Permanent deletes are not applied to our copy yet (mirrors the prior scan,
      # which never removed messages); reflecting trashes/deletes needs its own
      # data-safety pass. Surfaced here so the gap is visible, not silent.
      def log_deletes(ids)
        return if ids.blank?

        Rails.logger.info("[Emails::SyncStrategies::Google] #{account.email_address}: #{ids.size} provider deletes not applied (deferred)")
      end
    end
  end
end
