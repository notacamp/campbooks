require "rails_helper"

RSpec.describe "Settings::Security::Totp", type: :request do
  let(:user)   { create(:user) }
  let(:secret) { ROTP::Base32.random }

  before do
    allow(ROTP::Base32).to receive(:random).and_return(secret)
    sign_in(user) # authenticated; no MFA yet
  end

  def current_code = ROTP::TOTP.new(secret).now

  describe "GET new" do
    it "renders the QR and the manual key" do
      get new_settings_security_totp_path

      expect(response).to have_http_status(:ok)
      expect(response.body).to include(secret)
    end
  end

  describe "POST create" do
    it "enables TOTP with a valid code and mints recovery codes on the first factor" do
      get new_settings_security_totp_path # seeds session secret

      expect {
        post settings_security_totp_path, params: { code: current_code }
      }.to change { user.reload.totp_enabled_at }.from(nil)
        .and change { user.recovery_codes.count }.from(0).to(RecoveryCode::COUNT)

      expect(user.totp_secret).to eq(secret)
      expect(response).to have_http_status(:ok)
      expect(response.body).to match(/[a-z0-9]{5}-[a-z0-9]{5}/) # codes shown once
    end

    it "rejects an invalid code" do
      get new_settings_security_totp_path

      expect {
        post settings_security_totp_path, params: { code: "000000" }
      }.not_to change { user.reload.totp_enabled_at }

      expect(response).to have_http_status(:unprocessable_entity)
    end
  end

  describe "DELETE destroy" do
    before { user.update!(totp_secret: secret, totp_enabled_at: Time.current) }

    it "removes the TOTP factor when the password is confirmed" do
      delete settings_security_totp_path, params: { current_password: "password123" }

      expect(user.reload.totp_enabled_at).to be_nil
      expect(user.totp_secret).to be_nil
    end

    it "refuses removal without the correct password" do
      delete settings_security_totp_path, params: { current_password: "wrong" }

      expect(user.reload.totp_enabled_at).not_to be_nil
      expect(user.totp_secret).not_to be_nil
    end
  end
end
