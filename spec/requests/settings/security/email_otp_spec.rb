require "rails_helper"

RSpec.describe "Settings::Security::EmailOtp", type: :request do
  let(:user) { create(:user) }

  before { sign_in(user) } # no MFA yet → authenticated session

  describe "POST create" do
    it "enables email OTP and mints recovery codes on the first factor" do
      expect {
        post settings_security_email_otp_path, params: { current_password: "password123" }
      }.to change { user.reload.email_otp_enabled_at }.from(nil)
        .and change { user.recovery_codes.count }.from(0).to(RecoveryCode::COUNT)

      expect(response).to have_http_status(:ok)
      expect(response.body).to match(/[a-z0-9]{5}-[a-z0-9]{5}/)
    end
  end

  describe "DELETE destroy" do
    before { user.update!(email_otp_enabled_at: Time.current) }

    it "disables email OTP when the password is confirmed" do
      delete settings_security_email_otp_path, params: { current_password: "password123" }
      expect(user.reload.email_otp_enabled_at).to be_nil
    end

    it "refuses removal without the correct password" do
      delete settings_security_email_otp_path, params: { current_password: "wrong" }
      expect(user.reload.email_otp_enabled_at).not_to be_nil
    end
  end
end
