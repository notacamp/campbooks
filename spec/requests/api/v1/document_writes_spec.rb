require "rails_helper"

RSpec.describe "API v1 document writes", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }

  def write_headers
    api_auth_headers(workspace: workspace, user: user, scopes: "documents:write")
  end

  before do
    allow(Notifier).to receive(:documents_need_review)
    allow(Documents::FinalizeApprovalJob).to receive(:perform_later)
  end

  describe "PATCH /api/v1/documents/:id" do
    it "updates extracted fields" do
      doc = create(:document, workspace: workspace)

      patch api_v1_document_path(doc),
            params: { vendor_name: "Acme Ltd", amount_cents: 9999 }, headers: write_headers

      expect(response).to have_http_status(:ok)
      expect(doc.reload.vendor_name).to eq("Acme Ltd")
      expect(doc.amount_cents).to eq(9999)
    end

    it "403s with only the read scope" do
      doc = create(:document, workspace: workspace)

      patch api_v1_document_path(doc), params: { vendor_name: "X" },
            headers: api_auth_headers(workspace: workspace, user: user, scopes: "documents:read")

      expect(response).to have_http_status(:forbidden)
    end
  end

  describe "POST /api/v1/documents/:id/approve" do
    it "approves and records the reviewer" do
      doc = create(:document, :in_review, workspace: workspace)

      post approve_api_v1_document_path(doc), headers: write_headers

      expect(response).to have_http_status(:ok)
      expect(doc.reload.review_status).to eq("approved")
      expect(doc.reviewed_by).to eq(user)
      expect(Documents::FinalizeApprovalJob).to have_received(:perform_later).with(doc.id)
    end
  end

  describe "POST /api/v1/documents/:id/reject" do
    it "rejects the document" do
      doc = create(:document, :in_review, workspace: workspace)

      post reject_api_v1_document_path(doc), headers: write_headers

      expect(response).to have_http_status(:ok)
      expect(doc.reload.review_status).to eq("rejected")
    end
  end

  describe "POST /api/v1/documents/:id/reclassify" do
    it "changes the type and signs the document off" do
      doc = create(:document, :in_review, workspace: workspace)
      type = DocumentType.create!(workspace: workspace, name: "receipt", color: "#000", prompt: "x")

      post reclassify_api_v1_document_path(doc), params: { document_type_id: type.id }, headers: write_headers

      expect(response).to have_http_status(:ok)
      expect(doc.reload.document_type_id).to eq(type.id)
      expect(doc.review_status).to eq("approved")
    end

    it "404s for a document type in another workspace" do
      doc = create(:document, :in_review, workspace: workspace)
      type = DocumentType.create!(workspace: create(:workspace), name: "receipt", color: "#000", prompt: "x")

      post reclassify_api_v1_document_path(doc), params: { document_type_id: type.id }, headers: write_headers

      expect(response).to have_http_status(:not_found)
    end
  end
end
