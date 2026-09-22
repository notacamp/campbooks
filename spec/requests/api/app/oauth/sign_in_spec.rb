# frozen_string_literal: true

require "rails_helper"

# The unauthenticated SPA "Sign in with <provider>" start endpoints. Zoho is used
# for URL-building assertions because its client_id has a default (no CI KeyError).
RSpec.describe "API app OAuth sign-in start", type: :request do
  def state_from(url)
    ::Oauth::State.decode(Rack::Utils.parse_query(URI(url).query)["state"])
  end

  describe "GET /api/app/oauth/providers" do
    it "lists google + zoho without a token when microsoft is off" do
      allow(::Features).to receive(:microsoft?).and_return(false)

      get "/api/app/oauth/providers"

      expect(response).to have_http_status(:ok)
      expect(response.parsed_body.dig("data", "providers")).to contain_exactly("google", "zoho")
    end

    it "includes microsoft only when Features.microsoft?" do
      allow(::Features).to receive(:microsoft?).and_return(true)

      get "/api/app/oauth/providers"

      expect(response.parsed_body.dig("data", "providers")).to include("microsoft")
    end
  end

  describe "GET /api/app/oauth/sign_in_url" do
    it "returns an authorize URL carrying a verified spa sign_in state (no token)" do
      get "/api/app/oauth/sign_in_url", params: { provider: "zoho", return_to: "http://localhost:3100/today" }

      expect(response).to have_http_status(:ok)
      url = response.parsed_body.dig("data", "authorize_url")
      expect(url).to be_present
      expect(state_from(url)).to include("verified" => true, "spa" => true, "flow" => "sign_in",
                                         "return_to" => "http://localhost:3100/today")
    end

    it "422s for an unknown provider" do
      get "/api/app/oauth/sign_in_url", params: { provider: "carrier-pigeon" }

      expect(response).to have_http_status(:unprocessable_entity)
      expect(response.parsed_body.dig("error", "code")).to eq("invalid_provider")
    end

    it "404s microsoft when the feature is off" do
      allow(::Features).to receive(:microsoft?).and_return(false)

      get "/api/app/oauth/sign_in_url", params: { provider: "microsoft" }

      expect(response).to have_http_status(:not_found)
    end

    it "rejects an off-origin return_to (open-redirect guard) and falls back to the SPA login" do
      get "/api/app/oauth/sign_in_url", params: { provider: "zoho", return_to: "https://evil.example.com/steal" }

      return_to = state_from(response.parsed_body.dig("data", "authorize_url"))["return_to"]
      expect(return_to).to start_with("http://localhost:3100")
      expect(return_to).not_to include("evil.example.com")
    end
  end
end
