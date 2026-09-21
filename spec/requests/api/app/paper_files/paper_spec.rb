# frozen_string_literal: true

require "rails_helper"

RSpec.describe "API app Paper", type: :request do
  let(:workspace)  { create(:workspace) }
  let(:user)       { create(:user, workspace: workspace) }
  let(:other_workspace) { create(:workspace) }
  let(:other_user)      { create(:user, workspace: other_workspace) }

  describe "GET /api/app/paper" do
    let!(:invoice) { create(:document, workspace: workspace, document_type: :expense_invoice, review_status: :approved, ai_status: :completed) }
    let!(:receipt) { create(:document, :receipt, workspace: workspace, review_status: :approved, ai_status: :completed) }

    it "returns bucket counts and current bucket documents" do
      get "/api/app/paper", headers: api_app_headers(user)

      expect(response).to have_http_status(:ok)
      body = response.parsed_body["data"]
      expect(body["bucket"]).to eq("all")
      expect(body["bucket_counts"]).to include("all", "invoices", "receipts", "contracts", "other")
      expect(body["bucket_counts"]["all"]).to eq(2)
      expect(body["documents"]).to be_an(Array)
      expect(body["documents"].size).to eq(2)
    end

    it "filters by bucket" do
      get "/api/app/paper", params: { bucket: "receipts" }, headers: api_app_headers(user)

      body = response.parsed_body["data"]
      expect(body["bucket"]).to eq("receipts")
      expect(body["documents"].all? { |d| d["document_type"] == "receipt" }).to be true
    end

    it "returns invoices bucket count correctly" do
      get "/api/app/paper", params: { bucket: "invoices" }, headers: api_app_headers(user)

      body = response.parsed_body["data"]
      expect(body["bucket_counts"]["invoices"]).to eq(1)
    end

    it "does not include documents from another workspace" do
      other_doc = create(:document, workspace: other_workspace, review_status: :approved)

      get "/api/app/paper", headers: api_app_headers(user)

      ids = response.parsed_body.dig("data", "documents").map { |d| d["id"] }
      expect(ids).not_to include(other_doc.id)
    end

    it "requires authentication" do
      get "/api/app/paper"
      expect(response).to have_http_status(:unauthorized)
    end
  end
end
