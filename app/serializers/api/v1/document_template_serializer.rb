# frozen_string_literal: true

module Api
  module V1
    # Serializes a DocumentTemplate for the public API. List responses include
    # only the summary fields; pass detail: true (show / create / update) to
    # add html_content and variables_schema.
    class DocumentTemplateSerializer
      def initialize(template, detail: false)
        @template = template
        @detail = detail
      end

      def as_json
        data = {
          id: @template.id,
          name: @template.name,
          description: @template.description,
          ai_status: @template.ai_status,
          created_at: @template.created_at.iso8601,
          updated_at: @template.updated_at.iso8601
        }

        if @detail
          data[:html_content] = @template.html_content
          data[:variables_schema] = @template.variables_schema
        end

        data
      end
    end
  end
end
