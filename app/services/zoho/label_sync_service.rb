module Zoho
  class LabelSyncService
    MAX_LABELS_PER_RUN = 10

    def initialize(email_account)
      @account = email_account
      @client = email_account.mail_client
    end

    def sync_labels!
      zoho_labels = @client.list_labels
      zoho_label_ids = zoho_labels.map { |l| l["labelId"] || l["tagId"] }

      # Sync label definitions
      zoho_labels.each do |zl|
        external_id = zl["labelId"] || zl["tagId"]
        name = zl["displayName"]
        color = zl["color"].presence || Tag.palette_color_for(name)
        # Zoho exposes no system flag on /labels, so detect its built-in folders
        # by name (Inbox, Sent, …) — mirrors Gmail's system_label handling.
        sys = Labels::Classifier::ZOHO_SYSTEM_NAMES.include?(name.to_s.strip.downcase)

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
        # Reload to pick up hidden/classified_at set by update_columns inside
        # apply_classification!, which doesn't touch the in-memory record.
        tag.reload
        record_pending_decision(tag, name: name) unless tag.hidden?
      end

      # Destroy tags for labels deleted in Zoho
      @account.external_tags.where.not(external_label_id: zoho_label_ids).destroy_all

      # Reconcile assignments: Zoho → Campbooks (all labels, first page only)
      reconcile_assignments(@account.external_tags)

      @account.external_tags.count
    end

    private

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
      Rails.logger.warn("[Zoho::LabelSyncService] decision record failed for #{tag.external_label_id}: #{e.message}")
    end

    def reconcile_assignments(tags)
      tags.each do |tag|
        next if tag.hidden? # don't attach (or rate-limit on) hidden system/low-value labels

        reconcile_tag_assignments(tag)
        sleep(0.3) # Rate limit courtesy
      end
    end

    def reconcile_tag_assignments(tag)
      return if tag.hidden? # never attach hidden labels to messages

      # Zoho returns newest first — first page covers recent changes
      messages = @client.list_messages_by_label(tag.external_label_id, limit: 200)
      return if messages.empty?

      provider_message_ids = messages.map { |m| m["messageId"] }

      local_ids = @account.email_messages
        .where(provider_message_id: provider_message_ids)
        .pluck(:id, :provider_message_id)
        .to_h { |id, zid| [ zid, id ] }

      # Add assignments present in Zoho but missing locally
      provider_message_ids.each do |zoho_id|
        local_id = local_ids[zoho_id]
        next unless local_id
        EmailMessageTag.find_or_create_by!(email_message_id: local_id, tag_id: tag.id)
      end
    end
  end
end
