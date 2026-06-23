module Emails
  module SyncStrategies
    # Per-vendor email sync strategy. The engine (EmailScanJob) stays provider-
    # agnostic: it claims the scan slot, opens a scan log, and calls #sync! here.
    # Each subclass pulls changes the cheapest way its provider allows and routes
    # every message through Emails::MessageUpserter, so create/reconcile logic
    # lives in one place. Mirrors the calendar sync's incremental/full split.
    class Base
      def initialize(account)
        @account = account
      end

      # Cheap incremental pass, runs every minute. Pull changes since the stored
      # cursor, upsert them, advance the cursor; returns Result. Raises
      # Emails::CursorExpired if the provider rejects the cursor — the caller then
      # schedules a full re-baseline.
      def sync!(scan_log: nil)
        raise NotImplementedError, "#{self.class}#sync!"
      end

      # Heavy, rare pass. Walk every folder, ingest, and (re)establish the cursor.
      # Runs on first connect, on cursor expiry, and — for change-feed-less vendors
      # — on a periodic cadence to reconcile read/flag drift.
      def full_resync!(scan_log: nil)
        raise NotImplementedError, "#{self.class}#full_resync!"
      end

      # True when the provider exposes a real change feed (Gmail history, Graph
      # delta). Drives whether a periodic full resync is needed.
      def supports_delta? = false

      # Vendors without a change feed must periodically full-resync to catch read/
      # flag changes; delta vendors only resync to recover an expired cursor.
      def needs_periodic_resync? = !supports_delta?

      # True before the account has ever been baselined. The engine offloads the
      # (heavy) first walk to Emails::FullResyncJob rather than running it inline in
      # the minute poll. Folder-cursor vendors key off "no folders enumerated yet";
      # Gmail overrides to key off its account-wide historyId.
      def needs_bootstrap? = account.email_folders.none?

      private

      attr_reader :account

      def client
        @client ||= account.mail_client
      end

      def upserter(scan_log)
        Emails::MessageUpserter.new(account, scan_log: scan_log)
      end
    end
  end
end
