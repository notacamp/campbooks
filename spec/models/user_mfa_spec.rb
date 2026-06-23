require "rails_helper"

RSpec.describe User, "MFA", type: :model do
  let(:user) { create(:user) }

  describe "#mfa_enabled? / #mfa_methods" do
    it "is false with no factors" do
      expect(user.mfa_enabled?).to be(false)
      expect(user.mfa_methods).to eq([])
    end

    it "reports TOTP when enabled" do
      user.update!(totp_secret: ROTP::Base32.random, totp_enabled_at: Time.current)

      expect(user.mfa_enabled?).to be(true)
      expect(user.mfa_methods).to include(:totp)
    end

    it "reports email OTP when enabled" do
      user.update!(email_otp_enabled_at: Time.current)

      expect(user.mfa_methods).to include(:email_otp)
    end

    it "reports passkeys when a credential exists" do
      user.webauthn_credentials.create!(external_id: "abc", public_key: "key", sign_count: 0)

      expect(user.mfa_enabled?).to be(true)
      expect(user.mfa_methods).to include(:passkey)
    end

    it "orders methods totp, passkey, email_otp" do
      user.update!(totp_secret: ROTP::Base32.random, totp_enabled_at: Time.current,
                   email_otp_enabled_at: Time.current)
      user.webauthn_credentials.create!(external_id: "abc", public_key: "key", sign_count: 0)

      expect(user.mfa_methods).to eq(%i[totp passkey email_otp])
    end
  end

  describe "encrypted totp_secret" do
    it "round-trips through the app but is ciphertext at rest" do
      secret = ROTP::Base32.random
      user.update!(totp_secret: secret)

      expect(user.reload.totp_secret).to eq(secret)
      raw = User.connection.select_value("SELECT totp_secret FROM users WHERE id = #{user.id}")
      expect(raw).not_to eq(secret)
    end
  end

  describe "#ensure_webauthn_id!" do
    it "generates a stable handle once and reuses it" do
      first = user.ensure_webauthn_id!

      expect(first).to be_present
      expect(user.reload.webauthn_id).to eq(first)
      expect(user.ensure_webauthn_id!).to eq(first)
    end
  end
end
