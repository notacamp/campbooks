# frozen_string_literal: true

require "rails_helper"

RSpec.describe "API app onboarding", type: :request do
  let(:workspace) { create(:workspace, settings: {}) }
  let(:user) { create(:user, workspace: workspace) }
  let(:headers) { api_app_headers(user) }

  describe "GET /api/app/onboarding" do
    it "returns the onboarding state" do
      get "/api/app/onboarding", headers: headers

      expect(response).to have_http_status(:ok)
      data = response.parsed_body["data"]
      expect(data["completed"]).to be false
      expect(data["steps"]).to be_an(Array)
      expect(data["workspace"]).to be_a(Hash)
    end

    it "requires authentication" do
      get "/api/app/onboarding"
      expect(response).to have_http_status(:unauthorized)
    end
  end

  describe "PATCH /api/app/onboarding" do
    it "saves workspace step data and returns updated state" do
      patch "/api/app/onboarding", headers: headers,
                                   params: {
                                     step: "workspace",
                                     workspace: { name: "Acme Corp Updated" }
                                   }

      expect(response).to have_http_status(:ok)
      expect(response.parsed_body.dig("data", "workspace", "name")).to eq("Acme Corp Updated")
      expect(workspace.reload.name).to eq("Acme Corp Updated")
    end

    it "422s for an unknown step" do
      patch "/api/app/onboarding", headers: headers, params: { step: "not_a_real_step" }
      expect(response).to have_http_status(:unprocessable_entity)
      expect(response.parsed_body.dig("error", "code")).to eq("invalid_step")
    end

    it "marks onboarding completed when step is review" do
      patch "/api/app/onboarding", headers: headers, params: { step: "review" }

      expect(response).to have_http_status(:ok)
      expect(response.parsed_body.dig("data", "completed")).to be true
    end
  end

  describe "POST /api/app/onboarding/snooze" do
    it "marks the workspace as snoozed" do
      post "/api/app/onboarding/snooze", headers: headers

      expect(response).to have_http_status(:ok)
      expect(response.parsed_body.dig("data", "snoozed")).to be true
      expect(workspace.reload.settings["onboarding_snoozed_at"]).to be_present
    end
  end

  describe "GET /api/app/onboarding/first_sync_status" do
    it "returns the first sync status JSON" do
      get "/api/app/onboarding/first_sync_status", headers: headers

      expect(response).to have_http_status(:ok)
      # Onboarding::FirstSyncStatus#as_json returns state, stage?, found, etc.
      expect(response.parsed_body["data"]).to be_a(Hash)
    end
  end

  describe "POST /api/app/onboarding/apply_persona" do
    it "applies a valid template key and returns tag/doctype counts" do
      post "/api/app/onboarding/apply_persona", headers: headers,
                                                params: { template_keys: [ "freelancer" ] }

      expect(response).to have_http_status(:ok)
      data = response.parsed_body["data"]
      expect(data["applied_keys"]).to include("freelancer")
    end

    it "returns empty result when no valid keys are supplied" do
      post "/api/app/onboarding/apply_persona", headers: headers,
                                                params: { template_keys: [] }

      expect(response).to have_http_status(:ok)
      data = response.parsed_body["data"]
      expect(data["applied_keys"]).to be_empty
    end
  end

  describe "POST /api/app/onboarding/skip_first_sync" do
    it "marks the sync as skipped for this user" do
      post "/api/app/onboarding/skip_first_sync", headers: headers

      expect(response).to have_http_status(:ok)
      expect(response.parsed_body.dig("data", "skipped")).to be true
      expect(workspace.reload.settings["first_sync_skipped_by_user"]).to eq(user.id.to_s)
    end
  end
end
