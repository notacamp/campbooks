# frozen_string_literal: true

require "rails_helper"

RSpec.describe "API app setup cards", type: :request do
  let(:workspace) { create(:workspace, settings: {}) }
  let(:user) { create(:user, workspace: workspace) }
  let(:headers) { api_app_headers(user) }

  describe "GET /api/app/setup/:id" do
    it "returns the workspace step state" do
      get "/api/app/setup/workspace", headers: headers

      expect(response).to have_http_status(:ok)
      data = response.parsed_body["data"]
      expect(data["step"]).to eq("workspace")
      expect(data["workspace"]).to be_a(Hash)
    end

    it "returns the document_types step with presets" do
      get "/api/app/setup/document_types", headers: headers

      expect(response).to have_http_status(:ok)
      data = response.parsed_body["data"]
      expect(data["presets"]).to be_an(Array)
      expect(data["presets"].first).to include("name", "color")
    end

    it "returns the tags step with presets" do
      get "/api/app/setup/tags", headers: headers

      expect(response).to have_http_status(:ok)
      data = response.parsed_body["data"]
      expect(data["presets"]).to be_an(Array)
    end

    it "404s for an unknown step" do
      get "/api/app/setup/nonexistent", headers: headers
      expect(response).to have_http_status(:not_found)
    end

    it "requires authentication" do
      get "/api/app/setup/workspace"
      expect(response).to have_http_status(:unauthorized)
    end
  end

  describe "PATCH /api/app/setup/:id" do
    it "saves workspace data and returns updated state" do
      patch "/api/app/setup/workspace", headers: headers,
                                        params: { workspace: { name: "New Corp Name" } }

      expect(response).to have_http_status(:ok)
      expect(workspace.reload.name).to eq("New Corp Name")
    end
  end

  describe "POST /api/app/setup/dismiss" do
    it "adds the key to dismissed_setup_keys and returns { dismissed: true }" do
      post "/api/app/setup/dismiss", headers: headers, params: { key: "document_types" }

      expect(response).to have_http_status(:ok)
      data = response.parsed_body["data"]
      expect(data["dismissed"]).to be true
      expect(data["key"]).to eq("document_types")
      expect(workspace.reload.settings["dismissed_setup_keys"]).to include("document_types")
    end

    it "is idempotent — dismissing twice does not duplicate the key" do
      post "/api/app/setup/dismiss", headers: headers, params: { key: "tags" }
      post "/api/app/setup/dismiss", headers: headers, params: { key: "tags" }

      expect(workspace.reload.settings["dismissed_setup_keys"].count("tags")).to eq(1)
    end
  end
end
