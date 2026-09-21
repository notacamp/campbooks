# frozen_string_literal: true

module Api
  module App
    module Email
      # Single-message action dispatch via the EmailActions registry. Mirrors the
      # web EmailToolsController / v1 EmailActionsController but without Turbo
      # rendering or Doorkeeper scope gates — session bearer auth is enough here.
      # The client passes `tool` and an optional `args` hash; the response carries
      # success + message + a result payload suitable for an optimistic-update
      # cache patch.
      class ActionsController < BaseController
        def create
          email = ::EmailMessage.accessible_to(current_user).find(params[:email_message_id])
          tool  = params[:tool].to_s
          args  = extract_args

          outcome = ::EmailActions.run(tool, email_message: email, args: args, user: current_user)

          if outcome[:success]
            render_data({ success: true, tool: tool, message: outcome[:message], result: outcome[:result] })
          else
            render_error("action_failed", outcome[:message].presence || "Action failed.",
                         status: :unprocessable_entity)
          end
        end

        private

        def extract_args
          raw = params[:args]
          return {} if raw.blank?

          raw = JSON.parse(raw) if raw.is_a?(::String)
          raw = raw.to_unsafe_h if raw.respond_to?(:to_unsafe_h)
          (raw || {}).with_indifferent_access
        rescue JSON::ParserError
          {}
        end
      end
    end
  end
end
