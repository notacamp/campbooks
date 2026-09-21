# frozen_string_literal: true

module Api
  module App
    module Email
      # Thin wrapper over the public-API DraftSerializer. Adds nothing new — the
      # v1 DraftSerializer already includes dismissed_at-derived `dismissed` and
      # all composer fields. We keep this class so the /api/app surface has its
      # own serializer namespace and can diverge without touching the public API.
      class DraftSerializer
        def initialize(draft)
          @draft = draft
        end

        def as_json
          Api::V1::DraftSerializer.new(@draft).as_json
        end
      end
    end
  end
end
