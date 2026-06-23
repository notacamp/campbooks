require "rails_helper"

RSpec.describe "Settings::Security::RecoveryCodes", type: :request do
  let(:secret) { ROTP::Base32.random }
  let(:user)   { create(:user) }

  before do
    user.update!(totp_secret: secret, totp_enabled_at: Time.current)
    post session_path, params: { email_address: user.email_address, password: "password123" }
    post session_challenge_path, params: { method: "totp", code: ROTP::TOTP.new(secret).now }
  end

  describe "GET show" do
    it "shows the remaining count without revealing codes" do
      RecoveryCode.regenerate_for!(user)
      get settings_security_recovery_codes_path

      expect(response).to have_http_status(:ok)
      expect(response.body).to include(I18n.t("settings.security.recovery_codes.show.remaining", count: RecoveryCode::COUNT))
    end
  end

  describe "POST create" do
    it "regenerates codes and displays them once" do
      post settings_security_recovery_codes_path

      expect(user.recovery_codes.unused.count).to eq(RecoveryCode::COUNT)
      expect(response).to have_http_status(:ok)
      expect(response.body).to match(/[a-z0-9]{5}-[a-z0-9]{5}/)
    end

    it "invalidates the previous set" do
      old = RecoveryCode.regenerate_for!(user)
      post settings_security_recovery_codes_path

      expect(RecoveryCode.consume!(user, old.first)).to be_nil
    end
  end
end
