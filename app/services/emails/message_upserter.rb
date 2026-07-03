module Emails
  # The single ingest path for a provider message, shared by every sync strategy.
  # Given one normalized message hash — the shape every mail client's
  # #list_messages / delta method emits — it creates a new EmailMessage (and
  # enqueues processing) or reconciles read/flag on an existing one. Extracted
  # verbatim from EmailScanJob#scan_folder so the delta and full-resync paths
  # can never drift from each other.
  class MessageUpserter
    def self.upsert(account, msg, scan_log: nil)
      new(account, scan_log: scan_log).upsert(msg)
    end

    def initialize(account, scan_log: nil)
      @account = account
      @scan_log = scan_log
    end

    # `msg` keys (normalized by the mail clients): messageId, folderId,
    # fromAddress, toAddress, subject, summary, hasAttachment, receivedTime,
    # status ("1" = read, matching Zoho's convention), flagid, header_*.
    # Returns :created, :reconciled, :unchanged, :skipped, or :error. One bad
    # message is logged and skipped, never aborting the surrounding run (mirrors
    # the prior per-message rescue in EmailScanJob#scan_folder).
    def upsert(msg)
      message_id = msg["messageId"]
      return :skipped if message_id.blank?

      if (existing = @account.email_messages.find_by(provider_message_id: message_id))
        reconcile(existing, msg)
      else
        create(message_id, msg)
      end
    rescue => e
      Rails.logger.error("[Emails::MessageUpserter] #{@account.email_address} #{message_id}: #{e.message}")
      :error
    end

    private

    # Read is reconciled one-way (unread → read) and Zoho's flag is mirrored,
    # matching the prior full-sweep behaviour. (Two-way read/folder mirroring is a
    # deliberate follow-up; it needs the loop-avoidance the calendar sync has.)
    def reconcile(existing, msg)
      changed = false
      if msg["status"].to_s == "1" && !existing.read?
        existing.update_columns(read: true, updated_at: Time.current)
        changed = true
      end
      if @account.zoho? && msg["flagid"].present? && msg["flagid"] != existing.zoho_flag
        existing.update_columns(zoho_flag: msg["flagid"])
        changed = true
      end
      # Keep the provider label snapshot fresh (Gmail re-categorizations, user
      # label moves) — it feeds the triage category hint.
      labels = provider_labels(msg)
      if msg.key?("providerLabels") && labels != existing.provider_labels
        existing.update_columns(provider_labels: labels)
        changed = true
      end
      changed ? :reconciled : :unchanged
    end

    def create(message_id, msg)
      email_message = @account.email_messages.create!(
        email_scan_log: @scan_log,
        provider_message_id: message_id,
        # Provider-native conversation id (Gmail threadId / Graph conversationId);
        # nil for providers that don't expose one (Zoho). Drives threading.
        provider_thread_id: msg["providerThreadId"].presence,
        provider_folder_id: msg["folderId"],
        from_address: sanitize(msg["fromAddress"]),
        to_address: sanitize(msg["toAddress"]),
        subject: sanitize(msg["subject"]),
        summary: sanitize(msg["summary"]),
        has_attachment: msg["hasAttachment"].to_s == "1",
        received_at: msg["receivedTime"] ? Time.at(msg["receivedTime"].to_i / 1000) : nil,
        read: msg["status"].to_s == "1",
        zoho_flag: msg["flagid"],
        # Bulk/automated signals for Emails::Categorizer (nil where the provider
        # doesn't surface headers, e.g. Zoho's bulk view endpoint).
        header_list_unsubscribe: sanitize(msg["header_list_unsubscribe"]).presence,
        header_precedence: sanitize(msg["header_precedence"]).presence,
        header_auto_submitted: sanitize(msg["header_auto_submitted"]).presence,
        # Raw provider label ids (Gmail labelIds; [] elsewhere) — feeds the
        # triage category hint.
        provider_labels: provider_labels(msg),
        status: :fetched
      )
      EmailProcessJob.perform_later(email_message.id)
      :created
    rescue ActiveRecord::RecordNotUnique
      # Concurrent insert (overlapping scans claimed the same new message) — adopt
      # the winner and re-queue processing if it never got off the ground.
      existing = @account.email_messages.find_by!(provider_message_id: message_id)
      EmailProcessJob.perform_later(existing.id) if existing.fetched? || existing.failed?
      :created
    end

    # Strip PG-incompatible NUL bytes (mirrors ApplicationJob#sanitize_string).
    def sanitize(value)
      value.to_s.gsub("\u0000", "")
    end

    def provider_labels(msg)
      Array(msg["providerLabels"]).map { |label| sanitize(label) }
    end
  end
end
