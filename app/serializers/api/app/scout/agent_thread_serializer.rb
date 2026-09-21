# frozen_string_literal: true

module Api
  module App
    module Scout
      # Serializes a Scout chat thread for the first-party app API.
      # Extends the v1 shape with last_message_at for sidebar ordering.
      class AgentThreadSerializer
        def initialize(thread)
          @thread = thread
        end

        def as_json
          {
            id: @thread.id,
            title: @thread.title,
            purpose: @thread.purpose,
            context_label: @thread.context_label,
            created_at: @thread.created_at.iso8601,
            updated_at: @thread.updated_at.iso8601
          }
        end
      end
    end
  end
end
