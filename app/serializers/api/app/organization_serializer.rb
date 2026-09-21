# frozen_string_literal: true

module Api
  module App
    # Serializes an Organization for the /api/app surface. Includes people count,
    # email count, recent email thread stubs, and recent document stubs for the
    # detail view.
    class OrganizationSerializer
      def initialize(organization, detail: false, email_count: nil, document_count: nil,
                     recent_emails: [], recent_documents: [])
        @organization     = organization
        @detail           = detail
        @email_count      = email_count
        @document_count   = document_count
        @recent_emails    = recent_emails
        @recent_documents = recent_documents
      end

      def as_json
        data = {
          id:             @organization.id,
          name:           @organization.name,
          domain:         @organization.domain,
          notes:          @organization.try(:notes),
          people_count:   @organization.people.size,
          email_count:    @email_count || @organization.try(:email_count),
          document_count: @document_count || @organization.try(:document_count),
          created_at:     @organization.created_at.iso8601
        }

        if @detail
          data[:people]           = serialize_people
          data[:recent_emails]    = serialize_recent_emails
          data[:recent_documents] = serialize_recent_documents
        end

        data
      end

      private

      def serialize_people
        (@organization.people || []).map do |person|
          {
            id:    person.id,
            name:  person.name,
            email: person.try(:email) || person.contacts.first&.email
          }
        end
      end

      def serialize_recent_emails
        @recent_emails.map do |email|
          {
            id:          email.id,
            subject:     email.subject,
            from_address: email.from_address,
            received_at: email.received_at&.iso8601
          }
        end
      end

      def serialize_recent_documents
        @recent_documents.map do |doc|
          {
            id:            doc.id,
            title:         doc.display_title,
            document_type: doc.document_type,
            created_at:    doc.created_at.iso8601
          }
        end
      end
    end
  end
end
