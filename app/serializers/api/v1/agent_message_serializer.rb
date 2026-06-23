# frozen_string_literal: true

module Api
  module V1
    # Serializes a Scout message (AgentMessage) for the public API. Clients poll
    # for the async AI reply by watching for a message with author_type "ai" and
    # reply_status "replied". suggested_actions/prompts surface what Scout proposed.
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
          created_at: @message.created_at.iso8601
        }
      end
    end
  end
end
