# frozen_string_literal: true

module Api
  module App
    # Serializes an EmailMessage for the People conversation pane. Reuses the
    # same fields the email view renders; body is included always (the SPA shows
    # all messages in an open thread). Preload :contact, :email_account,
    # files_attachments: :blob on collections to avoid N+1.
    class MessageSerializer
      def initialize(message)
        @message = message
      end

      def as_json
        {
          id:           @message.id,
          subject:      @message.subject,
          from:         @message.from_address,
          to:           @message.to_address,
          cc:           @message.cc_address,
          sent:         @message.sent?,
          read:         @message.read,
          has_attachment: @message.has_attachment,
          received_at:  @message.received_at&.iso8601,
          thread_id:    @message.email_thread_id,
          account_id:   @message.email_account_id,
          body:         @message.body,
          summary:      @message.ai_summary,
          category:     @message.category,
          priority:     @message.ai_priority,
          attachments:  attachment_list
        }
      end

      private

      def attachment_list
        return [] unless @message.respond_to?(:files)

        @message.files.map do |att|
          { filename: att.filename.to_s, content_type: att.content_type, byte_size: att.byte_size }
        end
      rescue StandardError
        []
      end
    end
  end
end
