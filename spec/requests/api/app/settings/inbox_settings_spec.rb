# frozen_string_literal: true

require "rails_helper"

RSpec.describe "Inbox Settings API", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace, password: "secret1234", password_confirmation: "secret1234") }
  let(:headers) { api_app_headers(user) }

  # ── Tags ──────────────────────────────────────────────────────────────────

  describe "GET /api/app/inbox_settings/tags" do
    let!(:tag) { create(:tag, workspace: workspace, name: "Important") }

    it "returns workspace tags" do
      get "/api/app/inbox_settings/tags", headers: headers

      expect(response).to have_http_status(:ok)
      data = response.parsed_body["data"]
      expect(data).to have_key("visible")
      visible_names = data["visible"].map { |t| t["name"] }
      expect(visible_names).to include("Important")
    end

    it "401s without auth" do
      get "/api/app/inbox_settings/tags"
      expect(response).to have_http_status(:unauthorized)
    end
  end

  describe "POST /api/app/inbox_settings/tags" do
    it "creates a tag" do
      post "/api/app/inbox_settings/tags",
           params: { name: "NewTag", color: "#ff0000" },
           headers: headers

      expect(response).to have_http_status(:created)
      expect(workspace.tags.find_by(name: "NewTag")).to be_present
    end

    it "returns 422 for invalid tag" do
      # duplicate name
      create(:tag, workspace: workspace, name: "DupTag")
      post "/api/app/inbox_settings/tags",
           params: { name: "DupTag", color: "#ff0000" },
           headers: headers

      expect(response).to have_http_status(:unprocessable_entity)
    end
  end

  describe "PATCH /api/app/inbox_settings/tags/:id" do
    let!(:tag) { create(:tag, workspace: workspace) }

    it "updates the tag" do
      patch "/api/app/inbox_settings/tags/#{tag.id}",
            params: { name: "Updated" },
            headers: headers

      expect(response).to have_http_status(:ok)
      expect(tag.reload.name).to eq("Updated")
    end

    it "returns 404 for a tag in another workspace" do
      other_tag = create(:tag, workspace: create(:workspace))
      patch "/api/app/inbox_settings/tags/#{other_tag.id}",
            params: { tag: { name: "Hacked" } },
            headers: headers

      expect(response).to have_http_status(:not_found)
    end
  end

  describe "DELETE /api/app/inbox_settings/tags/:id" do
    let!(:tag) { create(:tag, workspace: workspace) }

    it "deletes the tag" do
      delete "/api/app/inbox_settings/tags/#{tag.id}", headers: headers
      expect(response).to have_http_status(:no_content)
      expect { tag.reload }.to raise_error(ActiveRecord::RecordNotFound)
    end
  end

  describe "PATCH /api/app/inbox_settings/tags/:id/toggle_hidden" do
    let!(:tag) { create(:tag, workspace: workspace, hidden: false) }

    it "toggles the tag hidden flag" do
      patch "/api/app/inbox_settings/tags/#{tag.id}/toggle_hidden", headers: headers

      expect(response).to have_http_status(:ok)
      expect(tag.reload.hidden).to be true
    end
  end

  # ── Rules ─────────────────────────────────────────────────────────────────

  describe "GET /api/app/inbox_settings/rules" do
    let!(:rule) { create(:email_rule, workspace: workspace, archive: true) }

    it "returns workspace rules" do
      get "/api/app/inbox_settings/rules", headers: headers

      expect(response).to have_http_status(:ok)
      data = response.parsed_body["data"]
      ids = data.map { |r| r["id"] }
      expect(ids).to include(rule.id)
    end
  end

  describe "POST /api/app/inbox_settings/rules" do
    it "creates a rule" do
      post "/api/app/inbox_settings/rules",
           params: { email_rule: { name: "Test Rule", criteria: { from: "boss@example.com" }, archive: true } },
           headers: headers

      expect(response).to have_http_status(:created)
      expect(workspace.email_rules.find_by(name: "Test Rule")).to be_present
    end
  end

  describe "PATCH /api/app/inbox_settings/rules/:id/toggle" do
    let!(:rule) { create(:email_rule, workspace: workspace, enabled: true, archive: true) }

    it "toggles the rule enabled state" do
      patch "/api/app/inbox_settings/rules/#{rule.id}/toggle", headers: headers

      expect(response).to have_http_status(:ok)
      expect(rule.reload.enabled).to be false
    end

    it "returns 404 for a rule in another workspace" do
      other_rule = create(:email_rule, workspace: create(:workspace), archive: true)
      patch "/api/app/inbox_settings/rules/#{other_rule.id}/toggle", headers: headers
      expect(response).to have_http_status(:not_found)
    end
  end

  # ── Document types ────────────────────────────────────────────────────────

  describe "GET /api/app/inbox_settings/document_types" do
    let!(:doc_type) { create(:document_type, workspace: workspace) }

    it "returns workspace document types" do
      get "/api/app/inbox_settings/document_types", headers: headers

      expect(response).to have_http_status(:ok)
      data = response.parsed_body["data"]
      ids = data.map { |t| t["id"] }
      expect(ids).to include(doc_type.id)
    end
  end
end
