# frozen_string_literal: true

module Api
  module V1
    # Serializes an EmailTemplate for the public API. List responses omit
    # body_html and variables_schema; pass detail: true (show / create / update)
    # to include them.
    class EmailTemplateSerializer
      def initialize(email_template, detail: false)
        @email_template = email_template
        @detail = detail
      end

      def as_json
        data = {
          id: @email_template.id,
          name: @email_template.name,
          description: @email_template.description,
          subject: @email_template.subject,
          ai_status: @email_template.ai_status,
          document_template_ids: @email_template.document_template_ids,
          created_at: @email_template.created_at.iso8601,
          updated_at: @email_template.updated_at.iso8601
        }

        if @detail
          data[:body_html] = @email_template.body_html
          data[:variables_schema] = @email_template.variable_definitions
        end

        data
      end
    end
  end
end
