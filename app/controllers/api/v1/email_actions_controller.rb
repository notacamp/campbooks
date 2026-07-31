# frozen_string_literal: true

module Api
  module V1
    # Single-message triage actions — archive, trash, snooze, pin, and the sender
    # allow/block/star actions — dispatched through the exact same EmailActions
    # registry the web UI drives (EmailToolsController). One execution path, one
    # set of permission checks; this controller only adds the OAuth-scope gate and
    # the JSON envelope, so the API and the web can never drift.
    #
    #   POST /api/v1/emails/:email_id/actions/:name   body: { args: {...} }
    #
    # The action name is a URL segment (:name), not a body param called :action —
    # Rails reserves params[:action] for the controller action, so a body key of
    # that name would be silently overwritten.
    class EmailActionsController < BaseController
      # API-exposed actions → the OAuth scope each requires. This allowlist is the
      # security boundary: only these registry actions are reachable over the API
      # (the registry also carries bulk/workflow/scout-only actions we don't
      # surface here), and each maps to the honest scope for what it mutates —
      # mailbox flags need emails:write, sending needs emails:send, and the
      # sender actions mutate the sender Contact so they need contacts:write.
      ACTION_SCOPES = {
        "archive"        => :"emails:write",
        "unarchive"      => :"emails:write",
        "trash"          => :"emails:write",
        "snooze"         => :"emails:write",
        "unsnooze"       => :"emails:write",
        "pin"            => :"emails:write",
        "unpin"          => :"emails:write",
        "forward_email"  => :"emails:send",
        "star_sender"    => :"contacts:write",
        "unstar_sender"  => :"contacts:write",
        "block_sender"   => :"contacts:write",
        "unblock_sender" => :"contacts:write",
        "allow_sender"   => :"contacts:write"
      }.freeze

      def create
        name = params[:name].to_s
        required_scope = ACTION_SCOPES[name]

        unless required_scope
          return render_api_error(
            "invalid_action",
            "Unknown action '#{name}'. Supported actions: #{ACTION_SCOPES.keys.join(', ')}.",
            status: :unprocessable_entity
          )
        end

        # Per-action scope gate. Raises Doorkeeper::Errors::TokenForbidden → 403
        # via BaseController's rescue_from, matching every other scoped endpoint.
        doorkeeper_authorize!(required_scope)

        email = EmailMessage.accessible_to(Current.user).find(params[:email_id])

        outcome = EmailActions.run(name, email_message: email, args: action_args, user: Current.user)

        if outcome[:success]
          render json: {
            data: EmailSerializer.new(email.reload, detail: true).as_json,
            meta: { action: name, result: outcome[:result] }
          }
        else
          # A mailbox send-permission failure (e.g. forwarding from an account the
          # acting user can't send on) is a 403; every other failure is a 422.
          status = send_permission_denied?(outcome) ? :forbidden : :unprocessable_entity
          render_api_error("action_failed", outcome[:message], status: status)
        end
      end

      private

      # Accepts a JSON object, a JSON string, or ActionController::Parameters —
      # action args are simple scalars (a tag name, a to_address, a snooze time),
      # never mass-assigned onto a model.
      def action_args
        raw = params[:args]
        raw = JSON.parse(raw) if raw.is_a?(String)
        raw = raw.to_unsafe_h if raw.respond_to?(:to_unsafe_h)
        (raw || {}).to_h.with_indifferent_access
      rescue JSON::ParserError
        {}
      end

      def send_permission_denied?(outcome)
        outcome[:message] == I18n.t("email_actions.send_permission_denied")
      end
    end
  end
end
