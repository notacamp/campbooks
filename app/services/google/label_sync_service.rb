module Google
  class LabelSyncService
    def initialize(email_account)
      @account = email_account
      @client = email_account.mail_client
    end

    def sync_labels!
      google_labels = @client.list_labels
      google_label_ids = google_labels.map { |l| l["id"] }

      google_labels.each do |gl|
        external_id = gl["id"]
        sys = @client.respond_to?(:system_label?) && @client.system_label?(external_id)

        # Humanize system-label names (CATEGORY_PERSONAL → "Personal") and use a
        # muted colour palette so they blend into the UI rather than competing with
        # user-created tags. Hidden by default in the inbox (see Tag.visible).
        name  = sys ? humanize_system_label(gl["name"]) : gl["name"]
        color = sys ? system_label_color(gl["name"])     : extract_color(gl)

        tag = Tag.find_or_initialize_by(
          email_account_id: @account.id,
          external_label_id: external_id
        )
        tag.assign_attributes(name: name, color: color, source: :external,
                              workspace: @account.workspace, system_label: sys)
        tag.save!
        Labels::ClassifyLabelJob.classify(tag)
        # Record a pending import decision only for user-visible labels — skip
        # any label that inline classification already hid (system / category).
        # We reload to pick up hidden/classified_at set by update_columns inside
        # apply_classification!, which doesn't touch the in-memory record.
        tag.reload
        record_pending_decision(tag, name: name) unless tag.hidden?
      end

      @account.external_tags.where.not(external_label_id: google_label_ids).destroy_all

      reconcile_assignments(@account.external_tags)

      @account.external_tags.count
    end

    private

    # ── System-label helpers ────────────────────────────────────────────────

    SYSTEM_LABEL_NAMES = {
      "INBOX" => "Inbox", "SENT" => "Sent", "DRAFT" => "Drafts",
      "TRASH" => "Trash", "SPAM" => "Spam", "UNREAD" => "Unread",
      "IMPORTANT" => "Important", "STARRED" => "Starred",
      "CHAT" => "Chat", "SNOOZED" => "Snoozed",
      "CATEGORY_PERSONAL" => "Personal", "CATEGORY_SOCIAL" => "Social",
      "CATEGORY_PROMOTIONS" => "Promotions", "CATEGORY_UPDATES" => "Updates",
      "CATEGORY_FORUMS" => "Forums"
    }.freeze

    SYSTEM_LABEL_COLORS = {
      "INBOX" => "#8FA4B0", "SENT" => "#8FA4B0", "DRAFT" => "#8FA4B0",
      "TRASH" => "#8FA4B0", "SPAM" => "#C49585", "UNREAD" => "#8FA4B0",
      "IMPORTANT" => "#C99D9D", "STARRED" => "#B8A870",
      "CHAT" => "#8FA4B0", "SNOOZED" => "#8FA4B0",
      "CATEGORY_PERSONAL" => "#8B9DC3", "CATEGORY_SOCIAL" => "#8BA89B",
      "CATEGORY_PROMOTIONS" => "#B0987A", "CATEGORY_UPDATES" => "#A898B8",
      "CATEGORY_FORUMS" => "#C4957A"
    }.freeze

    def humanize_system_label(name)
      SYSTEM_LABEL_NAMES[name] || name.titleize
    end

    def system_label_color(name)
      SYSTEM_LABEL_COLORS[name] || "#8FA4B0"
    end

    def extract_color(label)
      bg = label.dig("color", "backgroundColor")
      return Tag.palette_color_for(label["name"] || label["id"]) unless bg

      r = (bg["red"].to_f * 255).round.clamp(0, 255)
      g = (bg["green"].to_f * 255).round.clamp(0, 255)
      b = (bg["blue"].to_f * 255).round.clamp(0, 255)
      format("#%02x%02x%02x", r, g, b)
    end

    def reconcile_assignments(tags)
      tags.each do |tag|
        next if tag.hidden? # don't attach (or rate-limit on) hidden system/low-value labels

        reconcile_tag_assignments(tag)
        sleep(0.3)
      end
    end

    # Create a pending LabelImportDecision for a new user label so the workspace
    # review banner surfaces it. Uses find_or_create_by so repeated syncs are
    # idempotent and never overwrite a resolved decision.
    def record_pending_decision(tag, name:)
      LabelImportDecision.find_or_create_by!(
        email_account_id: @account.id,
        provider_label_id: tag.external_label_id
      ) do |dec|
        dec.provider_label_name = name
        dec.decision            = :pending
      end
    rescue => e
      Rails.logger.warn("[Google::LabelSyncService] decision record failed for #{tag.external_label_id}: #{e.message}")
    end

    def reconcile_tag_assignments(tag)
      return if tag.hidden? # never attach hidden labels to messages

      messages = @client.list_messages_by_label(tag.external_label_id, limit: 200)
      return if messages.empty?

      provider_message_ids = messages.map { |m| m["messageId"] }

      local_ids = @account.email_messages
        .where(provider_message_id: provider_message_ids)
        .pluck(:id, :provider_message_id)
        .to_h { |id, pid| [ pid, id ] }

      provider_message_ids.each do |gmail_id|
        local_id = local_ids[gmail_id]
        next unless local_id
        EmailMessageTag.find_or_create_by!(email_message_id: local_id, tag_id: tag.id)
      end
    end
  end
end
