# frozen_string_literal: true

require "rails_helper"

RSpec.describe "GET/PATCH /api/app/account", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user) do
    create(:user, workspace: workspace,
                  email_address: "test@example.com",
                  password: "secret1234",
                  password_confirmation: "secret1234",
                  locale: "en",
                  role: :member)
  end
  let(:headers) { api_app_headers(user) }

  describe "GET /api/app/account" do
    it "returns the current user profile" do
      get "/api/app/account", headers: headers

      expect(response).to have_http_status(:ok)
      data = response.parsed_body["data"]
      expect(data["email"]).to eq("test@example.com")
      expect(data["locale"]).to eq("en")
      expect(data).to have_key("compose_default")
    end

    it "401s without a token" do
      get "/api/app/account"
      expect(response).to have_http_status(:unauthorized)
    end
  end

  describe "PATCH /api/app/account/language" do
    it "updates the user locale" do
      patch "/api/app/account/language", params: { locale: "pt" }, headers: headers

      expect(response).to have_http_status(:ok)
      expect(user.reload.locale).to eq("pt")
      expect(response.parsed_body.dig("data", "locale")).to eq("pt")
    end
  end

  describe "PATCH /api/app/account/compose_preference" do
    it "updates compose_default" do
      patch "/api/app/account/compose_preference", params: { compose_default: "dock" }, headers: headers

      expect(response).to have_http_status(:ok)
      expect(user.reload.compose_default).to eq("dock")
    end
  end

  describe "PATCH /api/app/account/writing_style" do
    it "saves the writing style" do
      patch "/api/app/account/writing_style",
            params: { writing_style: "Formal and concise." },
            headers: headers

      expect(response).to have_http_status(:ok)
      expect(user.reload.writing_style).to eq("Formal and concise.")
    end
  end
end
