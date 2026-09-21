# frozen_string_literal: true

require "rails_helper"

RSpec.describe "API app passwords", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user) do
    create(:user,
           workspace: workspace,
           email_address: "pwd-#{SecureRandom.hex(4)}@example.com",
           password: "original123",
           password_confirmation: "original123")
  end

  describe "POST /api/app/passwords (request reset)" do
    before { user }  # ensure the account exists

    it "returns { sent: true } and enqueues the reset email when the address exists" do
      expect do
        post "/api/app/passwords",
             params: { email_address: user.email_address }
      end.to have_enqueued_mail(PasswordsMailer, :reset)

      expect(response).to have_http_status(:ok)
      expect(response.parsed_body.dig("data", "sent")).to be true
    end

    it "returns { sent: true } even for an unknown address (no user enumeration)" do
      expect do
        post "/api/app/passwords",
             params: { email_address: "nobody-#{SecureRandom.hex(6)}@example.com" }
      end.not_to have_enqueued_mail(PasswordsMailer, :reset)

      expect(response).to have_http_status(:ok)
      expect(response.parsed_body.dig("data", "sent")).to be true
    end
  end

  describe "PUT /api/app/passwords/:token (set new password)" do
    def reset_token
      user.password_reset_token
    end

    it "resets the password and destroys all sessions" do
      user.sessions.create!(user_agent: "old-device", ip_address: "1.2.3.4")

      put "/api/app/passwords/#{reset_token}",
          params: { password: "newpassword99", password_confirmation: "newpassword99" }

      expect(response).to have_http_status(:ok)
      expect(response.parsed_body.dig("data", "reset")).to be true
      expect(user.sessions.reload.count).to eq(0)
    end

    it "404s for an invalid or expired token" do
      put "/api/app/passwords/bad-token",
          params: { password: "newpassword99", password_confirmation: "newpassword99" }

      expect(response).to have_http_status(:not_found)
      expect(response.parsed_body.dig("error", "code")).to eq("invalid_token")
    end
  end
end
