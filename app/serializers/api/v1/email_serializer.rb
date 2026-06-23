# frozen_string_literal: true

module Api
  module V1
    # Serializes an EmailMessage for the public API. List responses omit the full
    # body; pass detail: true (the show endpoint) to include body + bcc.
    # Preload :tags on collections to avoid N+1 on #tag_names.
    class EmailSerializer
      def initialize(email, detail: false)
        @email = email
        @detail = detail
      end

      def as_json
        data = {
          id: @email.id,
          subject: @email.subject,
          from: @email.from_address,
          to: @email.to_address,
          cc: @email.cc_address,
          read: @email.read,
          has_attachment: @email.has_attachment,
          priority: @email.ai_priority,
          category: @email.category,
          summary: @email.ai_summary,
          pinned: @email.pinned_at.present?,
          received_at: @email.received_at&.iso8601,
          thread_id: @email.email_thread_id,
          account_id: @email.email_account_id,
          tags: @email.tag_names
        }

        if @detail
          data[:bcc] = @email.bcc_address
          data[:body] = @email.body
        end

        data
      end
    end
  end
end
