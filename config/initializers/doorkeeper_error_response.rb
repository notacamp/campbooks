# frozen_string_literal: true

# Unifies the OAuth token endpoint's error body with the rest of the public API.
# Doorkeeper's default is { error:, error_description:, state: }; we render the
# same envelope every /api/v1/* endpoint uses: { error: { code:, message: } }.
#
# This governs the *authorization server* (POST /api/oauth/token) errors such as
# invalid_client / invalid_scope / unsupported_grant_type. The *resource server*
# errors (invalid_token / insufficient_scope from doorkeeper_authorize!) are
# handled by rescue_from in Api::V1::BaseController, since handle_auth_errors is
# set to :raise. Both layers must match for a consistent client experience.
Rails.application.config.after_initialize do
  Doorkeeper::OAuth::ErrorResponse.class_eval do
    def body
      {
        error: {
          code: name.to_s,
          message: description.to_s
        }
      }
    end
  end
end
