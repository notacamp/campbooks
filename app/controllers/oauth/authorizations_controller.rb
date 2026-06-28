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

    rescue_from Doorkeeper::Errors::DoorkeeperError, with: :render_authorize_error

    private

    def render_authorize_error(exception)
      @authorize_error = exception.message
      render "doorkeeper/authorizations/error", status: :bad_request
    end
  end
end
