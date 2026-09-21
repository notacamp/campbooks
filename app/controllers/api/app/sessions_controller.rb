# frozen_string_literal: true

module Api
  module App
    # Session-bearer lifecycle for the first-party app API.
    #
    #   POST   /api/app/session                 password login
    #   DELETE /api/app/session                 sign out (revoke this bearer)
    #   POST   /api/app/oauth/native/exchange   redeem the native one-time token
    #
    # The MFA challenge *submit* (when login 401s with `mfa_required`) and the
    # whole registration / 2FA-management / onboarding surface belong to the auth
    # build agent (api_app_auth.rb). See api-migration/00-auth.md.
    class SessionsController < BaseController
      # Login + native exchange must be reachable WITHOUT an existing token.
      skip_before_action :authenticate_session_token!, only: %i[create native_exchange]

      # Per-IP brute-force throttle on login (mirrors the web SessionsController).
      rate_limit to: 10, within: 3.minutes, only: :create,
                 with: -> {
                   render_error("rate_limited", "Too many attempts. Try again later.",
                                status: :too_many_requests)
                 }

      # POST /api/app/session
      #   200 { data: { token, expires_at } }                       — signed in
      #   401 { error: { code: "mfa_required" }, mfa_token, methods } — needs 2FA
      #   401 { error: { code: "invalid_credentials" } }
      def create
        user = User.authenticate_by(login_params)

        if user.nil? || user.deletion_requested_at.present?
          return render_error("invalid_credentials", "Incorrect email or password.",
                              status: :unauthorized)
        end

        if user.mfa_enabled?
          render json: {
            error: { code: "mfa_required", message: "A second factor is required." },
            mfa_token: user.generate_token_for(:api_mfa_challenge),
            methods: user.mfa_methods.map(&:to_s)
          }, status: :unauthorized
        else
          issue_session_for(user)
        end
      end

      # DELETE /api/app/session — revoke the session behind this bearer.
      def destroy
        Current.session&.destroy
        head :no_content
      end

      # POST /api/app/oauth/native/exchange — the SPA/Capacitor twin of
      # SessionsController#native: swap the one-time :native_session token minted
      # by OauthNativeHandoff (campbooks://oauth?token=…) for a real bearer.
      def native_exchange
        user = User.find_by_token_for(:native_session, params[:token])

        if user.nil? || user.deletion_requested_at.present?
          return render_error("invalid_token", "This sign-in link has expired.",
                              status: :unauthorized)
        end

        issue_session_for(user)
      end

      private

      def login_params
        params.permit(:email_address, :password)
      end

      def issue_session_for(user)
        session = user.sessions.create!(user_agent: request.user_agent, ip_address: request.remote_ip)
        AuditEvent.log("sign_in", user: user, request: request)
        render_data({
          token: session.api_token,
          expires_at: (session.updated_at + Session::INACTIVITY_LIMIT).iso8601
        })
      end
    end
  end
end
