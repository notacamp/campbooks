module Tools
  class BulkArchive
    def self.call(args = {})
      # Gate to the acting user's readable accounts before any provider mutation.
      scope = EmailMessage.accessible_to(Current.user).where.not(status: :ignored)

      if args["email_ids"].present?
        scope = scope.where(id: args["email_ids"])
      else
        scope = apply_filters(scope, args)
      end

      # Move to archive folder on providers (grouped by account)
      scope.includes(:email_account).find_each.group_by(&:email_account).each do |account, messages|
        begin
          client = account.mail_client
          next unless client.respond_to?(:archive_folder_id)
          folder_id = client.archive_folder_id
          next unless folder_id

          provider_ids = messages.map(&:provider_message_id).compact
          next if provider_ids.empty?

          client.move_to_folder(provider_ids, folder_id)
          account.email_messages.where(id: messages.map(&:id)).update_all(
            provider_folder_id: folder_id,
            updated_at: Time.current
          )
        rescue => e
          Rails.logger.error("[Tools::BulkArchive] Failed for account #{account.id}: #{e.message}")
        end
      end

      ids = scope.ids
      count = ids.size
      # One summarizing event for the batch (not one per message) so the activity
      # feed stays readable. Best-effort + workspace-scoped to the acting user.
      Events.publish(
        "email.bulk_archived",
        workspace: Current.user&.workspace,
        payload: { "count" => count, "ids" => ids }
      ) if count.positive?

      { archived_count: count }
    end

    def self.apply_filters(scope, args)
      scope = scope.where(status: args["status"]) if args["status"].present?
      scope = scope.where(ai_priority: args["ai_priority"]) if args["ai_priority"].present?

      if args["tag_name"].present?
        scope = scope.joins(:tags).where(tags: { name: args["tag_name"].to_s.downcase.strip })
      end

      if args["date_from"].present?
        date = Date.parse(args["date_from"]) rescue nil
        scope = scope.where("received_at >= ?", date) if date
      end

      if args["date_to"].present?
        date = Date.parse(args["date_to"]) rescue nil
        scope = scope.where("received_at <= ?", date) if date
      end

      if args["contact_email"].present?
        scope = scope.where(from_address: args["contact_email"].to_s.downcase.strip)
      end

      scope
    end
  end
end
