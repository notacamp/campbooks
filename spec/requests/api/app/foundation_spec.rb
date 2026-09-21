# frozen_string_literal: true

require "rails_helper"

# Foundation of the first-party app API: the session-bearer identity resolver
# (Api::App::BaseController), the /me bootstrap, and the login/logout lifecycle.
RSpec.describe "API app foundation", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user) do
    create(:user, workspace: workspace, email_address: "z@example.com",
                  password: "secret1234", password_confirmation: "secret1234")
  end

  describe "GET /api/app/me" do
    it "returns the acting identity for a valid bearer token" do
      get "/api/app/me", headers: api_app_headers(user)

      expect(response).to have_http_status(:ok)
      body = response.parsed_body["data"]
      expect(body.dig("user", "email")).to eq("z@example.com")
      expect(body.dig("workspace", "id")).to eq(workspace.id)
      expect(body["features"]).to be_a(Hash)
      # time_zone must be the plain IANA string, not the ActiveSupport::TimeZone object
      expect(body.dig("user", "time_zone")).to be_a(String)
    end

    it "401s without a token" do
      get "/api/app/me"
      expect(response).to have_http_status(:unauthorized)
    end

    it "401s with a garbage token" do
      get "/api/app/me", headers: { "Authorization" => "Bearer not-a-real-token" }
      expect(response).to have_http_status(:unauthorized)
    end

    it "401s once the session is revoked" do
      session = api_app_session_for(user)
      headers = { "Authorization" => "Bearer #{session.api_token}" }
      session.destroy

      get "/api/app/me", headers: headers
      expect(response).to have_http_status(:unauthorized)
    end
  end

  describe "POST /api/app/session" do
    before { user } # ensure the account exists before we try to log in

    it "issues a working bearer token for correct credentials" do
      post "/api/app/session", params: { email_address: "z@example.com", password: "secret1234" }

      expect(response).to have_http_status(:ok)
      token = response.parsed_body.dig("data", "token")
      expect(token).to be_present

      get "/api/app/me", headers: { "Authorization" => "Bearer #{token}" }
      expect(response).to have_http_status(:ok)
    end

    it "401s with invalid_credentials for a wrong password" do
      post "/api/app/session", params: { email_address: "z@example.com", password: "wrong" }

      expect(response).to have_http_status(:unauthorized)
      expect(response.parsed_body.dig("error", "code")).to eq("invalid_credentials")
    end
  end

  describe "DELETE /api/app/session" do
    it "revokes the session behind the bearer" do
      session = api_app_session_for(user)
      headers = { "Authorization" => "Bearer #{session.api_token}" }

      delete "/api/app/session", headers: headers
      expect(response).to have_http_status(:no_content)

      get "/api/app/me", headers: headers
      expect(response).to have_http_status(:unauthorized)
    end
  end
end
