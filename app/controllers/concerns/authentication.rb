module Authentication
  extend ActiveSupport::Concern

  included do
    before_action :require_authentication
    helper_method :authenticated?
  end

  class_methods do
    def allow_unauthenticated_access(**options)
      skip_before_action :require_authentication, **options
    end
  end

  private
    def authenticated?
      resume_session
    end

    def require_authentication
      resume_session || request_authentication
    end

    # Bounce already-signed-in visitors away from the auth pages (sign in,
    # forgot password). Those views render under the default `application`
    # layout, so without this guard an authenticated user lands on the form
    # wrapped in the full app chrome (nav rail, topbar, drawers).
    def redirect_if_authenticated
      redirect_to root_path if authenticated?
    end

    def resume_session
      Current.session ||= find_session_by_cookie
    end

    def find_session_by_cookie
      return unless cookies.signed[:session_id]

      session = Session.find_by(id: cookies.signed[:session_id])
      return unless session

      # Idle past the inactivity window: drop the row + cookie and force re-auth.
      if session.expired?
        session.destroy
        cookies.delete(:session_id)
        return nil
      end

      session.touch_if_stale
      session
    end

    def request_authentication
      session[:return_to_after_authenticating] = request.url
      redirect_to "/session/new"
    end

    def after_authentication_url
      session.delete(:return_to_after_authenticating) || root_url
    end

    def start_new_session_for(user)
      user.sessions.create!(user_agent: request.user_agent, ip_address: request.remote_ip).tap do |session|
        Current.session = session
        cookies.signed.permanent[:session_id] = {
          value: session.id, httponly: true, same_site: :lax, secure: Rails.env.production?
        }
        AuditEvent.log("sign_in", user: user, request: request)
      end
    end

    # Identity is established (password verified, or OAuth/native sign-in
    # succeeded) but the user has a second factor. Park a short-lived, server-side
    # "pending MFA" marker (the Rails session cookie is encrypted) and hand off to
    # the challenge — the real session cookie is NOT issued until a factor passes
    # (see SessionChallengesController). Shared by password login and OAuth sign-in
    # so neither path can skip the second factor.
    def start_mfa_challenge_for(user)
      session[:pending_mfa] = {
        "user_id" => user.id,
        "password_verified_at" => Time.current.utc.iso8601,
        "methods" => user.mfa_methods.map(&:to_s)
      }
      redirect_to session_challenge_path
    end

    def terminate_session
      AuditEvent.log("sign_out", user: Current.session&.user, request: request)
      Current.session.destroy
      cookies.delete(:session_id)
    end
end
