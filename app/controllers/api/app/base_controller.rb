# frozen_string_literal: true

module Api
  module App
    # Base controller for the FIRST-PARTY app API (/api/app) that backs the React
    # SPA + Capacitor mobile app. Like Api::V1::BaseController it inherits from
    # ActionController::Base (NOT ApplicationController), so none of the app's
    # HTML cookie-session before_actions run — auth here is a SESSION BEARER
    # token, TOKEN-ONLY (user decision 2026-09-20):
    #
    #   Authorization: Bearer <Session#api_token>
    #
    # The token is Session#signed_id (tamper-proof, revocable by destroying the
    # row, no schema change). Resolving it sets Current.session → Current.user /
    # Current.workspace, after which the app's normal permission gates apply
    # unchanged (EmailMessage.accessible_to(Current.user), Current.workspace.<assoc>).
    #
    # Surface controllers subclass this and are declared in the per-surface
    # config/routes/api_app_*.rb files. See api-migration/README.md + 00-auth.md.
    class BaseController < ActionController::Base
      include Pagy::Backend

      # Bearer-token API: no cookie session ⇒ no CSRF token to verify, and JSON
      # request bodies must not be wrapped under a root key.
      skip_forgery_protection
      wrap_parameters false

      # Per-token throttle. The `by` lambda runs before auth resolves, so it keys
      # on the raw bearer (all requests for one session share a bucket) and falls
      # back to the request IP for tokenless calls.
      rate_limit to: 600, within: 1.minute,
                 by: -> { api_rate_limit_key },
                 with: -> {
                   render_error("rate_limited",
                                "Too many requests. Slow down and retry shortly.",
                                status: :too_many_requests)
                 }

      before_action :authenticate_session_token!
      around_action :use_user_locale

      rescue_from ActiveRecord::RecordNotFound, with: :render_not_found
      rescue_from ActiveRecord::RecordInvalid, with: :render_record_invalid
      rescue_from ActionController::ParameterMissing, with: :render_parameter_missing

      private

      # Resolve the Bearer token to its live Session and establish the acting
      # identity. Fails closed (401) when the token is missing/invalid/expired, or
      # the user is gone / pending deletion — mirrors Api::V1 establish_acting_identity!.
      def authenticate_session_token!
        session = Session.authenticate_api_token(bearer_token)
        user = session&.user

        return render_unauthenticated if user.nil?
        if user.deletion_requested_at.present?
          return render_error("account_pending_deletion",
                              "This account is scheduled for deletion.",
                              status: :unauthorized)
        end

        Current.session   = session
        Current.workspace = user.workspace
      end

      def render_unauthenticated
        render_error("unauthenticated", "Missing or invalid session token.", status: :unauthorized)
      end

      def bearer_token
        request.authorization.to_s.sub(/\ABearer\s+/i, "").presence
      end

      def current_user = Current.user
      def current_workspace = Current.workspace

      # Render every /api/app response in the acting user's locale (the SPA sends
      # no Accept-Language dance; the user's saved locale is the source of truth).
      def use_user_locale(&action)
        I18n.with_locale(Current.user&.locale.presence || I18n.default_locale, &action)
      end

      def api_rate_limit_key
        bearer_token || request.remote_ip
      end

      # ── response envelope (byte-compatible with /api/v1) ────────────────────

      # Single resource: { data: {...} }.
      def render_data(data, status: :ok)
        render json: { data: data }, status: status
      end

      # Paginated collection: { data: [...], meta: {...} }.
      def render_page(data, pagy)
        render json: { data: data, meta: pagy_meta(pagy) }
      end

      def pagy_meta(pagy)
        { page: pagy.page, per_page: pagy.limit, total: pagy.count, total_pages: pagy.pages }
      end

      # Per-page size for paginated endpoints: clamped to 1..100, default 25.
      def per_page
        requested = params[:per_page].to_i
        requested = 25 if requested <= 0
        [ requested, 100 ].min
      end

      def render_error(code, message, status:)
        render json: { error: { code: code, message: message } }, status: status
      end

      # 404-not-403 leak rule: a record in another workspace must look exactly
      # like one that does not exist. Never reveal existence across the tenant line.
      def render_not_found(*)
        render_error("not_found", "Not found.", status: :not_found)
      end

      def render_record_invalid(exception)
        render_error("invalid", exception.record.errors.full_messages.to_sentence,
                     status: :unprocessable_entity)
      end

      def render_parameter_missing(exception)
        render_error("parameter_missing", exception.message, status: :bad_request)
      end

      # Fail closed with 403 when the acting workspace's plan lacks `feature_key`
      # (mirrors the web EntitlementGuard / v1 require_entitlement!).
      def require_entitlement!(feature_key)
        return if Current.workspace&.entitlements&.feature?(feature_key)

        render_error("entitlement_required", "Your plan does not include this feature.",
                     status: :forbidden)
      end
    end
  end
end
