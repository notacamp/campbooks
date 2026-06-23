module Tools
  class QueryEmails
    def self.call(args = {})
      limit = [ args["limit"].to_i, 50 ].min
      limit = 20 if limit <= 0

      # Try semantic search if search_text is provided and embeddings exist
      if args["search_text"].present? && SearchRecord.by_type("EmailMessage").exists?
        filters = build_filters(args)
        results = SearchService.search(
          args["search_text"],
          workspace: Current.workspace,
          filters: filters.merge(searchable_type: "EmailMessage"),
          options: { limit: limit }
        )

        if results.any?
          message_ids = results.map(&:searchable_id)
          messages = EmailMessage.accessible_to(Current.user)
                                 .where(id: message_ids)
                                 .order(received_at: :desc)
                                 .limit(limit)
                                 .map { |msg| format_message(msg) }

          return { count: messages.size, messages: messages, search_method: "semantic" }
        end
      end

      # Fallback to ILIKE search. accessible_to gates to the acting user's
      # readable accounts (fails closed to none if there is no current user).
      scope = EmailMessage.accessible_to(Current.user)
      scope = apply_filters(scope, args)

      if args["search_text"].present?
        text = "%#{args["search_text"]}%"
        scope = scope.where("subject ILIKE ? OR body ILIKE ?", text, text)
      end

      total = scope.count
      messages = scope.order(received_at: :desc).limit(limit).map { |msg| format_message(msg) }

      { count: total, messages: messages, search_method: "text" }
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

      scope = scope.where(from_address: args["contact_email"].to_s.downcase.strip) if args["contact_email"].present?

      if args["has_attachment"].present?
        scope = scope.where(has_attachment: ActiveModel::Type::Boolean.new.cast(args["has_attachment"]))
      end

      scope
    end

    def self.build_filters(args)
      filters = {}
      filters[:status] = args["status"] if args["status"].present?
      filters[:tags] = [ args["tag_name"] ] if args["tag_name"].present?
      filters[:date_from] = args["date_from"] if args["date_from"].present?
      filters[:date_to] = args["date_to"] if args["date_to"].present?
      filters[:from_address] = args["contact_email"] if args["contact_email"].present?
      filters
    end

    def self.format_message(msg)
      {
        id: msg.id,
        subject: msg.subject,
        from_address: msg.from_address,
        received_at: msg.received_at&.iso8601,
        status: EmailMessage.statuses.key(msg.status),
        priority: EmailMessage.ai_priorities.key(msg.ai_priority),
        tags: msg.tags.pluck(:name),
        summary: msg.ai_summary&.truncate(200)
      }
    end
  end
end
