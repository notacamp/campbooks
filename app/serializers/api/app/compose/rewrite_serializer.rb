# frozen_string_literal: true

module Api
  module App
    module Compose
      # Round-trip shape for the AI tone-rewrite endpoint. The client sends
      # body_html + tone and gets back the rewritten body_html for the same tone
      # (so it can drop it straight into the editor).
      class RewriteSerializer
        def initialize(body_html, tone:)
          @body_html = body_html
          @tone = tone
        end

        def as_json
          { body_html: @body_html, tone: @tone }
        end
      end
    end
  end
end
