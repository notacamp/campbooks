module Tools
  # Aggregate snapshot injected into Scout's system prompt. Every metric is
  # scoped to the acting user's permitted data so the LLM never sees counts that
  # span other users' accounts or other workspaces.
  class SystemStats
    def self.call
      emails    = EmailMessage.accessible_to(Current.user)
      documents = Current.workspace&.documents || Document.none
      contacts  = Current.workspace&.contacts  || Contact.none

      email_statuses = emails.group(:status).count
      email_priorities = emails.group(:ai_priority).count
      tags = tag_distribution(Current.workspace)

      {
        emails: {
          total: emails.count,
          unread: emails.where(read: false).count,
          by_status: email_statuses.transform_keys { |k| EmailMessage.statuses.key(k) || k.to_s },
          by_priority: email_priorities.transform_keys { |k| EmailMessage.ai_priorities.key(k) || k.to_s },
          unique_senders: emails.distinct.count(:from_address),
          date_range: {
            oldest: emails.minimum(:received_at)&.iso8601,
            newest: emails.maximum(:received_at)&.iso8601
          }
        },
        documents: {
          total: documents.count,
          by_type: documents.group(:document_type).count.transform_keys { |k| Document.document_types.key(k) || k.to_s },
          by_ai_status: documents.group(:ai_status).count.transform_keys { |k| Document.ai_statuses.key(k) || k.to_s },
          by_review_status: documents.group(:review_status).count.transform_keys { |k| Document.review_statuses.key(k) || k.to_s }
        },
        contacts: {
          total: contacts.count,
          analyzed: contacts.where.not(analyzed_at: nil).count,
          with_person: contacts.where.not(person_id: nil).count
        },
        tags: {
          total: tags.size,
          top: tags.sort_by { |t| -t[:count] }.first(15)
        }
      }
    end

    def self.tag_distribution(workspace)
      return [] unless workspace

      Tag.where(workspace: workspace).joins(:email_message_tags)
        .group("tags.name", "tags.color")
        .count
        .map { |(name, color), count| { name: name, color: color, count: count } }
    end
  end
end
