# frozen_string_literal: true

module Api
  module App
    module Settings
      # Document type (classification).
      class DocumentTypeSerializer
        def initialize(doc_type)
          @type = doc_type
        end

        def as_json(*)
          {
            id: @type.id,
            name: @type.name,
            color: @type.color,
            category: @type.category,
            prompt: @type.prompt,
            auto_star: @type.auto_star,
            created_at: @type.created_at
          }
        end
      end
    end
  end
end
