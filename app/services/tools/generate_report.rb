module Tools
  class GenerateReport
    def self.call(args = {})
      type = args["type"] || args["report_type"] || "email_summary"
      date_from = args["date_from"].presence
      date_to = args["date_to"].presence

      case type
      when "email_summary"
        email_summary(date_from, date_to)
      when "document_summary"
        document_summary(date_from, date_to)
      when "contact_summary"
        contact_summary
      when "tag_distribution"
        tag_distribution
      else
        { error: "Unknown report type: #{type}. Available: email_summary, document_summary, contact_summary, tag_distribution" }
      end
    end

    def self.email_summary(date_from, date_to)
      scope = EmailMessage.accessible_to(Current.user)
      scope = scope.where("received_at >= ?", Date.parse(date_from)) if date_from
      scope = scope.where("received_at <= ?", Date.parse(date_to)) if date_to

      {
        type: "email_summary",
        total_emails: scope.count,
        unread: scope.where(read: false).count,
        by_status: scope.group(:status).count.transform_keys { |k| EmailMessage.statuses.key(k) || k.to_s },
        by_priority: scope.group(:ai_priority).count.transform_keys { |k| EmailMessage.ai_priorities.key(k) || k.to_s },
        period_start: date_from,
        period_end: date_to
      }
    end

    def self.document_summary(date_from, date_to)
      scope = Current.workspace&.documents || Document.none
      scope = scope.where("documents.metadata->>'document_date' >= ?", Date.parse(date_from).iso8601) if date_from
      scope = scope.where("documents.metadata->>'document_date' <= ?", Date.parse(date_to).iso8601) if date_to

      {
        type: "document_summary",
        total_documents: scope.count,
        by_type: scope.group(:document_type).count.transform_keys { |k| Document.document_types.key(k) || k.to_s },
        by_ai_status: scope.group(:ai_status).count.transform_keys { |k| Document.ai_statuses.key(k) || k.to_s },
        by_review_status: scope.group(:review_status).count.transform_keys { |k| Document.review_statuses.key(k) || k.to_s },
        total_amount_cents: scope.sum(Arel.sql("(CASE WHEN documents.metadata->>'amount_cents' ~ '^-{0,1}[0-9]{1,15}$' THEN (documents.metadata->>'amount_cents')::bigint END)")),
        period_start: date_from,
        period_end: date_to
      }
    end

    def self.contact_summary
      scope = Current.workspace&.contacts || Contact.none
      {
        type: "contact_summary",
        total_contacts: scope.count,
        by_relationship: scope.group(:relationship_type).count
      }
    end

    def self.tag_distribution
      return { type: "tag_distribution", tags: [] } unless Current.workspace

      dist = Tag.where(workspace: Current.workspace).joins(:email_message_tags)
        .group("tags.name", "tags.color")
        .order("count_all DESC")
        .limit(30)
        .count
        .map { |(name, color), count| { name: name, color: color, count: count } }

      { type: "tag_distribution", tags: dist }
    end
  end
end
