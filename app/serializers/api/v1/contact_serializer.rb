# frozen_string_literal: true

module Api
  module V1
    class ContactSerializer
      def initialize(contact)
        @contact = contact
      end

      def as_json
        {
          id: @contact.id,
          email: @contact.email,
          name: @contact.name,
          organization: @contact.organization,
          relationship_type: @contact.relationship_type,
          list_status: @contact.list_status,
          starred: @contact.starred?,
          email_count: @contact.email_count,
          last_email_at: @contact.last_email_at&.iso8601,
          context_summary: @contact.context_summary,
          person_id: @contact.person_id
        }
      end
    end
  end
end
