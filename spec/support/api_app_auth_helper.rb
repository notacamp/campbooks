# Helpers for exercising the first-party app API (/api/app) in request specs.
# Auth is a Session bearer (Session#api_token = signed_id), so authenticating is
# just minting a session for the user and sending its token.
module ApiAppAuthHelper
  def api_app_session_for(user)
    user.sessions.create!(user_agent: "rspec", ip_address: "127.0.0.1")
  end

  # Headers for a fresh session acting as `user` (or for an existing Session).
  def api_app_headers(user_or_session)
    session = user_or_session.is_a?(Session) ? user_or_session : api_app_session_for(user_or_session)
    { "Authorization" => "Bearer #{session.api_token}" }
  end
end

RSpec.configure do |config|
  config.include ApiAppAuthHelper, type: :request
end
