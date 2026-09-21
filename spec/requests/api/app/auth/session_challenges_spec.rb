# frozen_string_literal: true

require "rails_helper"

RSpec.describe "API app session challenges", type: :request do
  # Build a user who has MFA enabled via TOTP so we can exercise the full
  # mfa_token → challenge submit → bearer path.
  let(:workspace) { create(:workspace) }
  let(:user) do
    create(:user,
           workspace: workspace,
           email_address: "mfa-test-#{SecureRandom.hex(4)}@example.com",
           password: "supersecret99",
           password_confirmation: "supersecret99",
           totp_secret: ROTP::Base32.random,
           totp_enabled_at: ::Time.current,
           email_otp_enabled_at: ::Time.current)
  end

  def valid_mfa_token
    user.generate_token_for(:api_mfa_challenge)
  end

  describe "POST /api/app/session/challenge" do
    context "with TOTP" do
      it "issues a bearer token on a correct TOTP code" do
        token  = valid_mfa_token
        totp   = ROTP::TOTP.new(user.totp_secret)
        code   = totp.now

        post "/api/app/session/challenge",
             params: { mfa_token: token, method: "totp", code: code }

        expect(response).to have_http_status(:ok)
        expect(response.parsed_body.dig("data", "token")).to be_present
        expect(response.parsed_body.dig("data", "expires_at")).to be_present
      end

      it "returns invalid_code for a wrong TOTP code" do
        post "/api/app/session/challenge",
             params: { mfa_token: valid_mfa_token, method: "totp", code: "000000" }

        expect(response).to have_http_status(:unprocessable_entity)
        expect(response.parsed_body.dig("error", "code")).to eq("invalid_code")
      end
    end

    context "with email OTP" do
      let!(:challenge) do
        MfaEmailChallenge.create!(
          user: user,
          code_digest: BCrypt::Password.create("123456"),
          expires_at: 10.minutes.from_now
        )
      end

      it "issues a bearer token on a correct email OTP" do
        # Verify directly against the challenge's stored code.
        token = valid_mfa_token

        post "/api/app/session/challenge",
             params: { mfa_token: token, method: "email_otp", code: "123456" }

        expect(response).to have_http_status(:ok)
        expect(response.parsed_body.dig("data", "token")).to be_present
      end

      it "rejects an incorrect email OTP" do
        post "/api/app/session/challenge",
             params: { mfa_token: valid_mfa_token, method: "email_otp", code: "999999" }

        expect(response).to have_http_status(:unprocessable_entity)
      end
    end

    it "returns invalid_mfa_token for an expired / missing mfa_token" do
      post "/api/app/session/challenge",
           params: { mfa_token: "not-a-real-token", method: "totp", code: "123456" }

      expect(response).to have_http_status(:unauthorized)
      expect(response.parsed_body.dig("error", "code")).to eq("invalid_mfa_token")
    end

    it "returns invalid_mfa_token with no mfa_token at all" do
      post "/api/app/session/challenge",
           params: { method: "totp", code: "123456" }

      expect(response).to have_http_status(:unauthorized)
    end
  end

  describe "GET /api/app/session/challenge/options" do
    it "returns invalid_mfa_token for an unknown mfa_token" do
      get "/api/app/session/challenge/options", params: { mfa_token: "bad-token" }

      expect(response).to have_http_status(:unauthorized)
    end

    it "returns WebAuthn options JSON for a valid mfa_token (user with no passkeys)" do
      get "/api/app/session/challenge/options",
          params: { mfa_token: valid_mfa_token }

      # WebAuthn::Credential.options_for_get returns a valid JSON object.
      expect(response).to have_http_status(:ok)
      body = response.parsed_body
      expect(body["challenge"]).to be_present
    end
  end

  describe "POST /api/app/session/challenge/email_code" do
    it "sends the email OTP and returns { sent: true }" do
      expect do
        post "/api/app/session/challenge/email_code",
             params: { mfa_token: valid_mfa_token }
      end.to have_enqueued_mail(VerificationMailer, :verify)

      expect(response).to have_http_status(:ok)
      expect(response.parsed_body.dig("data", "sent")).to be true
    end

    it "returns invalid_mfa_token for an unknown token" do
      post "/api/app/session/challenge/email_code",
           params: { mfa_token: "bad-token" }

      expect(response).to have_http_status(:unauthorized)
    end
  end
end
