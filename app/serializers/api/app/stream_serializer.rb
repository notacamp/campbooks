# frozen_string_literal: true

module Api
  module App
    # Serializes one inbox-group stream row for the Streams tab.
    class StreamSerializer
      def initialize(stream_hash)
        @stream = stream_hash
      end

      def as_json
        {
          name:     @stream[:name],
          icon:     @stream[:icon],
          count:    @stream[:count],
          last_at:  @stream[:last_at]&.iso8601,
          note:     @stream[:note]
        }
      end
    end
  end
end
