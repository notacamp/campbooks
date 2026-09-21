# frozen_string_literal: true

module Api
  module App
    module Email
      # Bulk email/thread actions dispatched through Emails::BulkActions. Same
      # engine the web bulk toolbar and v1 use — one code path, one permission
      # boundary. The tool name is a body param (:tool) here (unlike v1 which
      # uses a URL segment) so the SPA can POST a plain JSON body.
      class BulkController < BaseController
        ALLOWED_TOOLS = ::Emails::BulkActions::TOOLS.freeze

        def create
          tool = params[:tool].to_s
          unless ALLOWED_TOOLS.include?(tool)
            return render_error("invalid_tool",
                                "Unknown bulk tool '#{tool}'. Allowed: #{ALLOWED_TOOLS.join(', ')}.",
                                status: :unprocessable_entity)
          end

          options = params.permit(:folder_id, :folder_name, :tag_name, :tag_action,
                                  :snoozed_until).to_h

          result = ::Emails::BulkActions.call(
            tool: tool,
            user: current_user,
            email_ids: Array(params[:email_ids]),
            groups: Array(params[:groups]),
            options: options
          )

          if result.empty_selection?
            render_error("empty_selection", "No messages matched the selection.",
                         status: :unprocessable_entity)
          elsif !result.ok?
            msg = result.error_message.presence || "Bulk action failed."
            render_error("bulk_action_failed", msg, status: :unprocessable_entity)
          else
            render_data({ tool: tool, count: result.all_ids.size })
          end
        end
      end
    end
  end
end
