module AuthHelper
  # Logs in through the real session endpoint so the signed session cookie is set
  # exactly as in production. The :user factory uses password "password123".
  def sign_in(user, password: "password123")
    post session_path, params: { email_address: user.email_address, password: password }
  end

  # Alias mirroring the Minitest ActionDispatch::IntegrationTest helper used by
  # migrated specs: signs out any live session first (SessionsController ignores
  # a login while one is active), and posts to the literal /session path — after
  # a request into a mounted engine (e.g. /jobs) URL helpers resolve against the
  # engine's router, so session_path would 404.
  def sign_in_as(user, password: "password123")
    delete "/session" if cookies[:session_id].present?
    post "/session", params: { email_address: user.email_address, password: password }
  end
end

RSpec.configure do |config|
  config.include AuthHelper, type: :request

  # Request specs aren't exercising onboarding; treat the workspace as set up so
  # ApplicationController#redirect_to_onboarding_if_incomplete doesn't 302 away
  # before the action under test runs.
  config.before(:each, type: :request) do
    allow_any_instance_of(SetupStatus).to receive(:complete?).and_return(true)
  end
end
