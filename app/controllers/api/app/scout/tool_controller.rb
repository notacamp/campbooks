# frozen_string_literal: true

module Api
  module App
    module Scout
      # Execute a confirm-level Scout tool on behalf of a user click.
      #
      # Only tools with autonomy :confirm may be dispatched here — this is the
      # prompt-injection firewall. The model proposes; only a human click executes.
      # Read-level tools (:read autonomy) are blocked because they are for the agent
      # loop only, not for direct user invocation.
      class ToolController < Api::App::BaseController
        # POST /api/app/scout/tool
        def create
          tool_name = params.require(:tool)
          args = tool_args

          tool_def = ::Scout::ToolRegistry.find(tool_name)
          unless tool_def&.confirm?
            return render_error("tool_not_allowed",
                                "Tool '#{tool_name}' is not available for direct execution.",
                                status: :unprocessable_entity)
          end

          result = ::Scout::ToolRegistry.run(tool_name, args)
          success = result.is_a?(Hash) && result[:error].blank?

          if success
            render_data(
              Api::App::Scout::ToolResultSerializer.new(
                tool: tool_name, success: true, result: result
              ).as_json
            )
          else
            render_error("tool_failed",
                         result.is_a?(Hash) ? result[:error].to_s : "Tool execution failed.",
                         status: :unprocessable_entity)
          end
        end

        private

        def tool_args
          raw = params[:args]
          return {} if raw.blank?
          raw.is_a?(String) ? JSON.parse(raw) : raw.to_unsafe_h
        rescue JSON::ParserError
          {}
        end
      end
    end
  end
end
