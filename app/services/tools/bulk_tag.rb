module Tools
  class BulkTag
    def self.call(args = {})
      tags = Current.workspace&.tags || Tag.none
      tag = tags.find_by("LOWER(name) = ?", args["tag_name"].to_s.downcase.strip)
      return { error: "Tag '#{args["tag_name"]}' not found" } unless tag

      action = args["action"] || "add"
      scope = EmailMessage.accessible_to(Current.user)

      if args["email_ids"].present?
        scope = scope.where(id: args["email_ids"])
      else
        scope = apply_filters(scope, args)
      end

      count = 0
      scope.find_each do |message|
        if action == "add"
          message.email_message_tags.find_or_create_by!(tag: tag)
          count += 1
        elsif action == "remove"
          assignment = message.email_message_tags.find_by(tag: tag)
          if assignment&.destroy!
            count += 1
          end
        end
      end

      { tagged_count: count, tag_name: tag.name, action: action }
    end

    def self.apply_filters(scope, args)
      scope = scope.where(status: args["status"]) if args["status"].present?
      scope = scope.where(ai_priority: args["ai_priority"]) if args["ai_priority"].present?

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
