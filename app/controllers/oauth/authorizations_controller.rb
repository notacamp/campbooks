# frozen_string_literal: true

module Oauth
  # Browser SSO consent screen for the public API's authorization_code + PKCE flow
  # (used by the Campbooks CLI). Inherits Doorkeeper's authorize/approve/deny
  # actions and the cookie-session resource-owner lookup wired in
  # config/initializers/doorkeeper.rb; we only swap in the app's minimal auth
  # layout, a styled consent view (app/views/doorkeeper/authorizations/new.html.erb),
  # and a friendly error page.
  #
  # handle_auth_errors is :raise globally because the API token + resource-server
  # endpoints need it to render the { error: { code, message } } JSON envelope.
  # That also makes Doorkeeper raise (rather than render) on a malformed authorize
  # request, so we catch it here and show browser users a readable page instead of
  # a 500.
  class AuthorizationsController < Doorkeeper::AuthorizationsController
    layout "doorkeeper"

    # On approval the consent form POSTs here and the server 302s to the CLI's
    # loopback callback (http://127.0.0.1:<port>/callback, RFC 8252). Browsers
    # enforce `form-action` across that redirect chain, so the global policy
    # (:self + the mailbox OAuth providers) would block it. Widen form-action to
    # the loopback origins on this controller only — the sole form here is the
    # consent form, so this can't enable cross-origin form posting elsewhere.
    content_security_policy do |policy|
      policy.form_action :self,
                         "http://127.0.0.1:*", "http://localhost:*",
                         "https://127.0.0.1:*", "https://localhost:*"
    end

    rescue_from Doorkeeper::Errors::DoorkeeperError, with: :render_authorize_error

    private

    def render_authorize_error(exception)
      @authorize_error = exception.message
      render "doorkeeper/authorizations/error", status: :bad_request
    end
  end
end
