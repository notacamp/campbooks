# frozen_string_literal: true

module Api
  module App
    module Scout
      # Serializes a Scout message for the first-party app API.
      # Extends the v1 shape with viewed_at so the SPA can manage the unread dot.
      class AgentMessageSerializer
        def initialize(message)
          @message = message
        end

        def as_json
          {
            id: @message.id,
            thread_id: @message.agent_thread_id,
            author_type: @message.author_type,
            content: @message.content,
            reply_status: @message.reply_status,
            suggested_actions: @message.ai_suggested_actions,
            prompts: @message.ai_prompts,
            viewed_at: @message.viewed_at&.iso8601,
            created_at: @message.created_at.iso8601
          }
        end
      end
    end
  end
end
