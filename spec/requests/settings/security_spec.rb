require "rails_helper"

RSpec.describe "Settings::Security", type: :request do
  let(:secret) { ROTP::Base32.random }
  let(:user)   { create(:user) }

  # Authenticate a user who has TOTP on by completing the login challenge.
  def sign_in_with_totp
    user.update!(totp_secret: secret, totp_enabled_at: Time.current)
    post session_path, params: { email_address: user.email_address, password: "password123" }
    post session_challenge_path, params: { method: "totp", code: ROTP::TOTP.new(secret).now }
  end

  describe "GET show" do
    it "renders the security hub for an authenticated user" do
      sign_in(user) # no MFA yet → straight authenticated session
      get settings_security_path

      expect(response).to have_http_status(:ok)
      expect(response.body).to include(I18n.t("settings.security.show.title"))
    end
  end

  describe "DELETE disable" do
    before { sign_in_with_totp }

    it "wipes every factor with the correct password" do
      RecoveryCode.regenerate_for!(user)

      delete disable_settings_security_path, params: { current_password: "password123" }

      expect(response).to redirect_to(settings_security_path)
      user.reload
      expect(user.mfa_enabled?).to be(false)
      expect(user.recovery_codes.count).to eq(0)
    end

    it "rejects a wrong password and keeps 2FA on" do
      delete disable_settings_security_path, params: { current_password: "nope" }

      expect(user.reload.mfa_enabled?).to be(true)
    end
  end
end
