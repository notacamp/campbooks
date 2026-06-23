# frozen_string_literal: true

module Api
  module V1
    class DocumentTypeSerializer
      def initialize(document_type)
        @document_type = document_type
      end

      def as_json
        {
          id: @document_type.id,
          name: @document_type.name,
          color: @document_type.color,
          category: @document_type.category,
          auto_star: @document_type.auto_star,
          extraction_schema: @document_type.extraction_schema
        }
      end
    end
  end
end
