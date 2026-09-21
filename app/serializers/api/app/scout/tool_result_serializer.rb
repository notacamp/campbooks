# frozen_string_literal: true

module Api
  module App
    module Scout
      # Serializes the result of a confirm-level tool execution.
      # Used by POST /api/app/scout/tool.
      class ToolResultSerializer
        def initialize(tool:, success:, result:)
          @tool = tool
          @success = success
          @result = result
        end

        def as_json
          {
            tool: @tool,
            success: @success,
            result: @result,
            toast_message: toast_message
          }
        end

        private

        def toast_message
          return nil unless @success && @result.is_a?(Hash)

          case @tool
          when "bulk_archive"
            "Archived #{@result[:archived_count]} email(s)"
          when "bulk_tag"
            verb = @result[:action] == "remove" ? "Removed" : "Added"
            "#{verb} tag '#{@result[:tag_name]}' on #{@result[:tagged_count]} email(s)"
          when "reclassify"
            "Re-classified #{@result[:reclassified_count]} email(s)"
          else
            "Done"
          end
        end
      end
    end
  end
end
