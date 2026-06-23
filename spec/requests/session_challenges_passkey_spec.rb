require "rails_helper"
require "webauthn/fake_client"

RSpec.describe "Passkey login challenge", type: :request do
  let(:user)   { create(:user) }
  let(:client) { WebAuthn::FakeClient.new("http://localhost:3000") }

  # Register a passkey while authenticated (no MFA yet), then sign out.
  before do
    post session_path, params: { email_address: user.email_address, password: "password123" }
    get options_settings_security_passkeys_path(format: :json)
    registration = client.create(challenge: JSON.parse(response.body)["challenge"])
    post settings_security_passkeys_path, params: { credential: registration.to_json, nickname: "Key" }
    delete session_path
  end

  it "registers exactly one credential during setup" do
    expect(user.webauthn_credentials.count).to eq(1)
  end

  it "signs in with a passkey assertion" do
    post session_path, params: { email_address: user.email_address, password: "password123" }
    expect(response).to redirect_to(session_challenge_path)

    get passkey_options_session_challenge_path(format: :json)
    assertion = client.get(challenge: JSON.parse(response.body)["challenge"])

    post session_challenge_path, params: { method: "passkey", credential: assertion.to_json }
    expect(response).to redirect_to(root_url)
    expect(user.webauthn_credentials.first.last_used_at).to be_present
  end

  it "rejects a missing/garbled assertion" do
    post session_path, params: { email_address: user.email_address, password: "password123" }
    get passkey_options_session_challenge_path(format: :json)

    post session_challenge_path, params: { method: "passkey", credential: "{}" }
    expect(response).to have_http_status(:unprocessable_entity)
  end
end
