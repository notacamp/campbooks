# frozen_string_literal: true

module Api
  module V1
    # Base controller for the public REST API. Deliberately inherits from
    # ActionController::Base (NOT ApplicationController) so none of the app's
    # cookie-session / HTML before_actions run; authentication here is OAuth
    # bearer tokens only (Doorkeeper).
    #
    # Auth flow per request:
    #   1. rate_limit            — per API client (falls back to IP if tokenless)
    #   2. doorkeeper_authorize! — valid, unexpired, unrevoked bearer token
    #   3. require_granted_scopes — token must carry at least one scope
    #   4. establish_acting_identity! — set Current.workspace + Current.acting_user
    #
    # After step 4 the app's normal permission gates apply unchanged
    # (EmailMessage.accessible_to(Current.user), Current.workspace.documents, …).
    # Subclasses add per-action scope checks, e.g.:
    #   before_action -> { doorkeeper_authorize! :"emails:send" }, only: :create
    class BaseController < ActionController::Base
      include Doorkeeper::Rails::Helpers
      include Pagy::Backend

      # Bearer-token API: no cookie session, so no CSRF token to verify, and
      # incoming JSON should not be wrapped under a root key.
      skip_forgery_protection
      wrap_parameters false

      # Per-client throttle. The `by` lambda runs before auth, so it tolerates a
      # missing/invalid token by falling back to the request IP.
      rate_limit to: 600, within: 1.minute,
                 by: -> { doorkeeper_token&.application_id&.to_s || request.remote_ip },
                 with: -> {
                   render_api_error("rate_limit_exceeded",
                                    "Too many requests. Slow down and retry shortly.",
                                    status: :too_many_requests)
                 }

      before_action :doorkeeper_authorize!
      before_action :require_granted_scopes
      before_action :establish_acting_identity!

      rescue_from ActiveRecord::RecordNotFound, with: :render_not_found
      rescue_from ActiveRecord::RecordInvalid, with: :render_record_invalid
      rescue_from ActionController::ParameterMissing, with: :render_parameter_missing
      # Invalid/expired/revoked/unknown token → 401. TokenExpired/Revoked/Unknown
      # all subclass InvalidToken, so the one handler covers them.
      rescue_from Doorkeeper::Errors::InvalidToken, with: :render_invalid_token
      # Valid token but missing scope → 403. TokenForbidden < InvalidToken, and
      # rescue_from checks the most recently registered handler first, so this
      # MUST stay after the InvalidToken handler above.
      rescue_from Doorkeeper::Errors::TokenForbidden, with: :render_insufficient_scope

      private

      # Bridge the resource-owner-less client_credentials token to the workspace
      # and user it represents. Fails closed (401) if the client's workspace or
      # acting user has gone away or the account is pending deletion.
      def establish_acting_identity!
        application = doorkeeper_token&.application
        workspace   = application&.workspace
        acting_user = application&.created_by

        unless workspace && acting_user && acting_user.workspace_id == workspace.id
          return render_api_error("client_revoked",
                                  "This API client is no longer valid.",
                                  status: :unauthorized)
        end

        if acting_user.respond_to?(:deletion_requested_at) && acting_user.deletion_requested_at.present?
          return render_api_error("account_pending_deletion",
                                  "This account is scheduled for deletion.",
                                  status: :unauthorized)
        end

        Current.workspace   = workspace
        Current.acting_user = acting_user
      end

      # A token issued without a `scope` param carries no scopes and can do
      # nothing — surface that explicitly rather than 403-ing on every endpoint
      # with no explanation (the most common client mistake).
      def require_granted_scopes
        return unless Array(doorkeeper_token&.scopes).empty?

        render_api_error("insufficient_scope",
                         "This token has no scopes. Pass a `scope` parameter when requesting the token.",
                         status: :forbidden)
      end

      def current_user
        Current.user
      end

      # ---- response helpers -------------------------------------------------

      # Single resource: { data: {...} }. `data` is an already-serialized hash.
      def render_data(data, status: :ok)
        render json: { data: data }, status: status
      end

      # Paginated collection: { data: [...], meta: {...} }. `data` is an array of
      # already-serialized hashes; `pagy` is the Pagy instance from `pagy(scope)`.
      def render_page(data, pagy)
        render json: { data: data, meta: pagy_meta(pagy) }
      end

      def pagy_meta(pagy)
        {
          page: pagy.page,
          per_page: pagy.limit,
          total: pagy.count,
          total_pages: pagy.pages
        }
      end

      # Per-page size for paginated endpoints: clamped to 1..100, default 25.
      def per_page
        requested = params[:per_page].to_i
        requested = 25 if requested <= 0
        [ requested, 100 ].min
      end

      # ---- error helpers ----------------------------------------------------

      def render_api_error(code, message, status:, details: nil)
        body = { error: { code: code, message: message } }
        body[:error][:details] = details if details
        render json: body, status: status
      end

      def render_not_found(_error = nil)
        render_api_error("not_found", "Resource not found.", status: :not_found)
      end

      def render_record_invalid(error)
        render_api_error("validation_failed", "Validation failed.",
                         status: :unprocessable_entity, details: error.record.errors.as_json)
      end

      def render_parameter_missing(error)
        render_api_error("missing_parameter", error.message, status: :bad_request)
      end

      def render_invalid_token(_error)
        render_api_error("invalid_token", "The access token is invalid or expired.",
                         status: :unauthorized)
      end

      def render_insufficient_scope(_error)
        render_api_error("insufficient_scope",
                         "This token lacks the required scope for this action.",
                         status: :forbidden)
      end
    end
  end
end
