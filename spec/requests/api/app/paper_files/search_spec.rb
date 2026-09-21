# frozen_string_literal: true

require "rails_helper"

RSpec.describe "API app Search", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace) }

  describe "GET /api/app/search" do
    it "returns search results" do
      get "/api/app/search", params: { q: "invoice" }, headers: api_app_headers(user)

      expect(response).to have_http_status(:ok)
      body = response.parsed_body["data"]
      expect(body).to have_key("results")
      expect(body).to have_key("grouped")
      expect(body).to have_key("total")
      expect(body["results"]).to be_an(Array)
    end

    it "returns empty results for very short queries" do
      get "/api/app/search", params: { q: "x" }, headers: api_app_headers(user)

      expect(response).to have_http_status(:ok)
      expect(response.parsed_body.dig("data", "results")).to eq([])
    end

    it "accepts a types filter" do
      get "/api/app/search", params: { q: "acme", types: [ "contacts" ] }, headers: api_app_headers(user)

      expect(response).to have_http_status(:ok)
    end

    it "requires authentication" do
      get "/api/app/search", params: { q: "test" }
      expect(response).to have_http_status(:unauthorized)
    end
  end
end
