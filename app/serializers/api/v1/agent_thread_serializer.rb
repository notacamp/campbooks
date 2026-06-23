# frozen_string_literal: true

module Api
  module V1
    # Serializes a Scout chat thread (AgentThread) for the public API.
    class AgentThreadSerializer
      def initialize(thread)
        @thread = thread
      end

      def as_json
        {
          id: @thread.id,
          title: @thread.title,
          purpose: @thread.purpose,
          created_at: @thread.created_at.iso8601,
          updated_at: @thread.updated_at.iso8601
        }
      end
    end
  end
end
