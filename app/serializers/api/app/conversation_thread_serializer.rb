# frozen_string_literal: true

module Api
  module App
    # Serializes a People::ConversationThread. When the thread is loaded (messages
    # present) all messages are included; when lazy (header only), only the metadata
    # needed to render the collapsed row is returned. The SPA mirrors the web app:
    # first thread is eager, subsequent threads are fetched on expand.
    class ConversationThreadSerializer
      def initialize(conversation_thread)
        @ct = conversation_thread
      end

      def as_json
        data = {
          id:         @ct.thread.id,
          subject:    @ct.subject,
          count:      @ct.count,
          latest_at:  @ct.latest_at&.iso8601,
          newest_id:  @ct.newest_id,
          loaded:     @ct.loaded?
        }

        if @ct.loaded?
          data[:messages] = @ct.messages.map { |m| Api::App::MessageSerializer.new(m).as_json }
        end

        data
      end
    end
  end
end
