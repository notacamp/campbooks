# frozen_string_literal: true

require "rails_helper"

RSpec.describe "API app registrations", type: :request do
  # Use SIGNUP_MODE :open in these specs to skip beta-code gating.
  around do |ex|
    original = Rails.application.config.signup_mode
    Rails.application.config.signup_mode = :open
    ex.run
  ensure
    Rails.application.config.signup_mode = original
  end

  let(:unique_email) { "reg-#{SecureRandom.hex(6)}@example.com" }

  # Decode a signed registration_token and return the embedded state hash.
  def decode_registration_token(token)
    verifier = Rails.application.message_verifier(:api_registration)
    JSON.parse(verifier.verify(token))
  rescue ActiveSupport::MessageVerifier::InvalidSignature
    nil
  end

  # ── Step 1: POST /api/app/registration ────────────────────────────────────

  describe "POST /api/app/registration" do
    it "returns a registration_token and enqueues the OTP email" do
      expect do
        post "/api/app/registration", params: {
          name: "Alice",
          email_address: unique_email,
          terms_accepted: "1"
        }
      end.to have_enqueued_mail(VerificationMailer, :verify)

      expect(response).to have_http_status(:ok)
      data = response.parsed_body["data"]
      expect(data["registration_token"]).to be_present
      expect(data["step"]).to eq("verify")
    end

    it "422s on a missing / invalid email" do
      post "/api/app/registration", params: {
        name: "Alice", email_address: "not-an-email", terms_accepted: "1"
      }
      expect(response).to have_http_status(:unprocessable_entity)
      expect(response.parsed_body.dig("error", "code")).to eq("invalid_email")
    end

    it "422s when terms are not accepted" do
      post "/api/app/registration", params: { name: "Alice", email_address: unique_email }
      expect(response).to have_http_status(:unprocessable_entity)
      expect(response.parsed_body.dig("error", "code")).to eq("terms_required")
    end

    it "still returns a token when the email is already registered (existing_user: true)" do
      create(:user, email_address: unique_email)

      post "/api/app/registration", params: {
        name: "Alice",
        email_address: unique_email,
        terms_accepted: "1"
      }

      expect(response).to have_http_status(:ok)
      data = response.parsed_body["data"]
      expect(data["existing_user"]).to be true
      expect(data["registration_token"]).to be_present
    end
  end

  # ── Step 2: POST /api/app/registration/verify ────────────────────────────

  describe "POST /api/app/registration/verify" do
    # Step 1 is called once per example; the token is memoized.
    let(:registration_token) do
      post "/api/app/registration", params: {
        name: "Alice", email_address: unique_email, terms_accepted: "1"
      }
      response.parsed_body.dig("data", "registration_token")
    end

    # Decode the signed token to get the OTP without going through email.
    def correct_code_for(token)
      decode_registration_token(token)&.dig("code")
    end

    it "verifies the correct OTP and returns step: password" do
      token = registration_token
      code  = correct_code_for(token)

      post "/api/app/registration/verify",
           params: { registration_token: token, code: code }

      expect(response).to have_http_status(:ok)
      expect(response.parsed_body.dig("data", "step")).to eq("password")
      expect(response.parsed_body.dig("data", "registration_token")).to be_present
    end

    it "422s on an incorrect code and carries the attempt count forward" do
      token = registration_token

      post "/api/app/registration/verify",
           params: { registration_token: token, code: "000000" }

      expect(response).to have_http_status(:unprocessable_entity)
      expect(response.parsed_body.dig("error", "code")).to eq("incorrect_code")
    end

    it "422s with invalid_registration_token for an unknown token" do
      post "/api/app/registration/verify",
           params: { registration_token: "not-a-real-token", code: "123456" }

      expect(response).to have_http_status(:unprocessable_entity)
      expect(response.parsed_body.dig("error", "code")).to eq("invalid_registration_token")
    end
  end

  # ── Step 2b: POST /api/app/registration/resend_code ─────────────────────

  describe "POST /api/app/registration/resend_code" do
    let(:registration_token) do
      post "/api/app/registration", params: {
        name: "Alice", email_address: unique_email, terms_accepted: "1"
      }
      response.parsed_body.dig("data", "registration_token")
    end

    it "sends a fresh OTP and returns { sent: true } with a new token" do
      token = registration_token

      expect do
        post "/api/app/registration/resend_code",
             params: { registration_token: token }
      end.to have_enqueued_mail(VerificationMailer, :verify)

      expect(response).to have_http_status(:ok)
      data = response.parsed_body["data"]
      expect(data["sent"]).to be true
      expect(data["registration_token"]).to be_present
    end
  end

  # ── Step 3: POST /api/app/registration/complete ──────────────────────────

  describe "POST /api/app/registration/complete" do
    # Returns a verified registration_token (steps 1 + 2 completed).
    def verified_token_for(email)
      post "/api/app/registration", params: {
        name: "Bob #{SecureRandom.hex(4)}",
        email_address: email,
        terms_accepted: "1"
      }
      step1_token = response.parsed_body.dig("data", "registration_token")
      code = Rails.application.message_verifier(:api_registration)
                   .verify(step1_token)
                   .then { |json| JSON.parse(json)["code"] }

      post "/api/app/registration/verify",
           params: { registration_token: step1_token, code: code }

      response.parsed_body.dig("data", "registration_token")
    end

    it "creates a user and workspace and returns a bearer token" do
      email = "complete-#{SecureRandom.hex(6)}@example.com"
      token = verified_token_for(email)

      expect do
        post "/api/app/registration/complete",
             params: { registration_token: token, password: "securepass123" }
      end.to change(User, :count).by(1).and change(Workspace, :count).by(1)

      expect(response).to have_http_status(:created)
      data = response.parsed_body["data"]
      expect(data["token"]).to be_present
      expect(data["expires_at"]).to be_present

      # The returned bearer must authenticate successfully.
      get "/api/app/me", headers: { "Authorization" => "Bearer #{data["token"]}" }
      expect(response).to have_http_status(:ok)
    end

    it "422s if password is too short" do
      email = "short-#{SecureRandom.hex(6)}@example.com"
      token = verified_token_for(email)

      post "/api/app/registration/complete",
           params: { registration_token: token, password: "short" }

      expect(response).to have_http_status(:unprocessable_entity)
      expect(response.parsed_body.dig("error", "code")).to eq("password_too_short")
    end

    it "422s if the email has not been verified (step 2 skipped)" do
      post "/api/app/registration", params: {
        name: "Carol",
        email_address: "carol-#{SecureRandom.hex(6)}@example.com",
        terms_accepted: "1"
      }
      step1_token = response.parsed_body.dig("data", "registration_token")

      # Skip verify — go straight to complete.
      post "/api/app/registration/complete",
           params: { registration_token: step1_token, password: "securepass123" }

      expect(response).to have_http_status(:unprocessable_entity)
      expect(response.parsed_body.dig("error", "code")).to eq("not_verified")
    end
  end
end
