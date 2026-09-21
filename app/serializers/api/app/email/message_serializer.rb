# frozen_string_literal: true

module Api
  module App
    module Email
      # Screen-shaped email message for the React SPA. Extends the public-API
      # EmailSerializer with body HTML, thread chain, folder memberships, tag
      # chips, and per-message dismiss state. Pass detail: true on the show
      # endpoint; pass thread_messages: [...] to include the thread chain.
      class MessageSerializer
        def initialize(message, detail: false, thread_messages: nil)
          @message = message
          @detail = detail
          @thread_messages = thread_messages
        end

        def as_json
          data = {
            id: @message.id,
            subject: @message.subject,
            from: @message.from_address,
            to: @message.to_address,
            cc: @message.cc_address,
            read: @message.read,
            has_attachment: @message.has_attachment,
            priority: @message.ai_priority,
            category: @message.category,
            summary: @message.ai_summary,
            pinned: @message.pinned_at.present?,
            received_at: @message.received_at&.iso8601,
            thread_id: @message.email_thread_id,
            account_id: @message.email_account_id,
            tags: @message.tag_names,
            folder_ids: folder_ids,
            contact_id: @message.contact_id,
            dismissed_todo: @message.ai_todo_dismissed
          }

          if @detail
            data[:bcc] = @message.bcc_address
            data[:body] = @message.body
            data[:thread_messages] = thread_message_stubs if @thread_messages
          end

          data
        end

        private

        def folder_ids
          @message.respond_to?(:folder_memberships) ? @message.folder_memberships.map(&:mail_folder_id) : []
        rescue
          []
        end

        def thread_message_stubs
          @thread_messages.map do |m|
            {
              id: m.id,
              from: m.from_address,
              read: m.read,
              received_at: m.received_at&.iso8601,
              has_attachment: m.has_attachment,
              tags: m.tag_names,
              dismissed_todo: m.ai_todo_dismissed
            }
          end
        end
      end
    end
  end
end
