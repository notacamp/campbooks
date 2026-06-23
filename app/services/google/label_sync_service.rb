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
        name = gl["name"]
        color = extract_color(gl)

        tag = Tag.find_or_initialize_by(
          email_account_id: @account.id,
          external_label_id: external_id
        )
        tag.assign_attributes(name: name, color: color, source: :external)
        tag.save!
      end

      @account.external_tags.where.not(external_label_id: google_label_ids).destroy_all

      reconcile_assignments(@account.external_tags)

      @account.external_tags.count
    end

    private

    def extract_color(label)
      bg = label.dig("color", "backgroundColor")
      return "#ffd700" unless bg

      r = (bg["red"].to_f * 255).round.clamp(0, 255)
      g = (bg["green"].to_f * 255).round.clamp(0, 255)
      b = (bg["blue"].to_f * 255).round.clamp(0, 255)
      format("#%02x%02x%02x", r, g, b)
    end

    def reconcile_assignments(tags)
      tags.each do |tag|
        reconcile_tag_assignments(tag)
        sleep(0.3)
      end
    end

    def reconcile_tag_assignments(tag)
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
