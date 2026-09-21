# frozen_string_literal: true

module Api
  module App
    module Scout
      # Serializes the Scout overlay idle-state payload.
      # Used by GET /api/app/scout/overlay.
      class OverlaySerializer
        def initialize(ai_available:, briefing:, recent_threads:)
          @ai_available = ai_available
          @briefing = briefing
          @recent_threads = recent_threads
        end

        def as_json
          {
            ai_available: @ai_available,
            briefing: {
              greeting: @briefing[:greeting],
              subtitle: @briefing[:subtitle],
              stats: @briefing[:stats],
              suggestions: @briefing[:suggestions]
            },
            recent_threads: @recent_threads.map { |t| AgentThreadSerializer.new(t).as_json }
          }
        end
      end
    end
  end
end
