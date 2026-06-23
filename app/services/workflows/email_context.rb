module Workflows
  # Trigger context for the `email_received` trigger. Exposes the email and its
  # documents to Liquid templates and conditions.
  class EmailContext < TriggerContext
    attr_reader :email_message

    def initialize(email_message)
      @email_message = email_message
    end

    def documents
      @email_message.documents
    end

    # The email is the record this trigger is about, so an emit_event action in
    # an email-triggered workflow records its new event against it.
    def subject
      @email_message
    end

    def liquid_context
      {
        "email" => {
          "from" => @email_message.from_address.to_s,
          "to" => @email_message.to_address.to_s,
          "subject" => @email_message.subject.to_s,
          "body" => @email_message.body.to_s,
          "received_at" => @email_message.received_at&.iso8601,
          "account_email" => @email_message.email_account&.email_address.to_s
        },
        "documents" => document_drops
      }
    end

    def trigger_data
      {
        "type" => "email_received",
        "email_message_id" => @email_message.id,
        "subject" => @email_message.subject.to_s,
        "from" => @email_message.from_address.to_s
      }
    end

    def step_input
      { "email_message_id" => @email_message.id }
    end

    private

    def document_drops
      @email_message.documents.map do |doc|
        {
          "filename" => doc.original_file&.filename.to_s,
          "document_type" => (doc.classification&.name || doc.document_type),
          "status" => doc.display_status,
          "ai_status" => doc.ai_status,
          "review_status" => doc.review_status
        }
      end
    end
  end
end
