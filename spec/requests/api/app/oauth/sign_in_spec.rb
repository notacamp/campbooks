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
    # No APP_FRONTEND_URL set: the SPA is same-origin with the API (prod +
    # self-hosted), so the frontend origin is derived from the request host,
    # which is www.example.com in request specs.
    def stub_frontend_env(value)
      allow(ENV).to receive(:[]).and_call_original
      allow(ENV).to receive(:[]).with("APP_FRONTEND_URL").and_return(value)
    end

    it "returns an authorize URL carrying a verified spa sign_in state (no token)" do
      stub_frontend_env(nil)

      get "/api/app/oauth/sign_in_url", params: { provider: "zoho", return_to: "http://www.example.com/today" }

      expect(response).to have_http_status(:ok)
      url = response.parsed_body.dig("data", "authorize_url")
      expect(url).to be_present
      expect(state_from(url)).to include("verified" => true, "spa" => true, "flow" => "sign_in",
                                         "return_to" => "http://www.example.com/today")
    end

    it "honours a return_to under an explicitly configured APP_FRONTEND_URL" do
      stub_frontend_env("https://app.campbooks.example/")

      get "/api/app/oauth/sign_in_url", params: { provider: "zoho", return_to: "https://app.campbooks.example/today" }

      expect(state_from(response.parsed_body.dig("data", "authorize_url"))["return_to"])
        .to eq("https://app.campbooks.example/today")
    end

    it "derives the SPA origin from the request when APP_FRONTEND_URL is unset (prod/self-hosted same-origin)" do
      # Regression: a hardcoded localhost:3100 default sent every prod/self-hosted
      # sign-in token to a dead local URL. The request host must win instead.
      stub_frontend_env(nil)

      get "/api/app/oauth/sign_in_url", params: { provider: "zoho", return_to: "https://app.campbooks.example/today" }

      # return_to is off the request origin (www.example.com) → rewritten to it,
      # NOT to localhost:3100.
      return_to = state_from(response.parsed_body.dig("data", "authorize_url"))["return_to"]
      expect(return_to).to start_with("http://www.example.com")
      expect(return_to).not_to include("localhost:3100")
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
      stub_frontend_env(nil)

      get "/api/app/oauth/sign_in_url", params: { provider: "zoho", return_to: "https://evil.example.com/steal" }

      return_to = state_from(response.parsed_body.dig("data", "authorize_url"))["return_to"]
      expect(return_to).to start_with("http://www.example.com")
      expect(return_to).not_to include("evil.example.com")
    end
  end
end
