# Helpers for exercising the public REST API (Doorkeeper bearer tokens) in
# request specs. Token secrets are hashed at rest, so the Authorization header
# uses #plaintext_token, which is only populated on the freshly-created instance.
module ApiAuthHelper
  def api_application_for(workspace:, user:, scopes: "emails:read")
    create(:api_application, workspace: workspace, created_by: user, scopes: scopes)
  end

  def issue_api_token(application:, scopes: nil)
    create(:api_access_token, application: application, scopes: scopes || application.scopes.to_s)
  end

  def api_headers(token)
    { "Authorization" => "Bearer #{token.plaintext_token}" }
  end

  # One-liner: headers for a fresh client acting as `user` in `workspace`.
  def api_auth_headers(workspace:, user:, scopes: "emails:read")
    application = api_application_for(workspace: workspace, user: user, scopes: scopes)
    api_headers(issue_api_token(application: application, scopes: scopes))
  end
end

RSpec.configure do |config|
  config.include ApiAuthHelper, type: :request
end
