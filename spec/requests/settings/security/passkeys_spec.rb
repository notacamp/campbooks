require "rails_helper"
require "webauthn/fake_client"

RSpec.describe "Settings::Security::Passkeys", type: :request do
  let(:user)   { create(:user) }
  let(:client) { WebAuthn::FakeClient.new("http://localhost:3000") }

  before { sign_in(user) } # no MFA yet → authenticated session

  def register(nickname: "My Key")
    get options_settings_security_passkeys_path(format: :json)
    challenge = JSON.parse(response.body)["challenge"]
    credential = client.create(challenge: challenge)
    post settings_security_passkeys_path, params: { credential: credential.to_json, nickname: nickname }
  end

  describe "POST create" do
    it "registers a passkey and mints recovery codes on the first factor" do
      expect { register }
        .to change { user.webauthn_credentials.count }.by(1)
        .and change { user.recovery_codes.count }.from(0).to(RecoveryCode::COUNT)

      expect(user.webauthn_credentials.last.nickname).to eq("My Key")
    end

    it "rejects a malformed credential" do
      get options_settings_security_passkeys_path(format: :json)

      expect {
        post settings_security_passkeys_path, params: { credential: "not json" }
      }.not_to change { user.webauthn_credentials.count }

      expect(response).to redirect_to(new_settings_security_passkey_path)
    end
  end

  describe "DELETE destroy" do
    it "removes a passkey when the password is confirmed" do
      register
      passkey = user.webauthn_credentials.last

      expect { delete settings_security_passkey_path(passkey), params: { current_password: "password123" } }
        .to change { user.webauthn_credentials.count }.by(-1)
    end

    it "refuses removal without the correct password" do
      register
      passkey = user.webauthn_credentials.last

      expect { delete settings_security_passkey_path(passkey), params: { current_password: "wrong" } }
        .not_to change { user.webauthn_credentials.count }
    end
  end
end
