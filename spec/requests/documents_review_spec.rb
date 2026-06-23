require "rails_helper"

# The list-view quick-action endpoints for the two-axis review model: approve/reject
# sign a document off (or junk it) from the documents list, and reprocess re-runs AI.
RSpec.describe "Documents review actions", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }

  before do
    sign_in(user)
    allow(Documents::FinalizeApprovalJob).to receive(:perform_later)
    allow(DocumentProcessJob).to receive(:perform_later)
  end

  describe "POST /documents/:id/approve" do
    it "marks the document review-approved and answers a Turbo Stream row swap" do
      doc = create(:document, :in_review, workspace: workspace)

      post approve_document_path(doc), headers: { "Accept" => "text/vnd.turbo-stream.html" }

      expect(response).to have_http_status(:ok)
      expect(response.media_type).to eq("text/vnd.turbo-stream.html")
      expect(doc.reload).to be_review_approved
      expect(Document.needs_review).not_to include(doc)
      expect(Documents::FinalizeApprovalJob).to have_received(:perform_later).with(doc.id)
    end
  end

  describe "POST /documents/:id/reject" do
    it "marks the document review-rejected and drops it from the review queue" do
      doc = create(:document, :in_review, workspace: workspace)

      post reject_document_path(doc), headers: { "Accept" => "text/vnd.turbo-stream.html" }

      expect(response).to have_http_status(:ok)
      expect(doc.reload).to be_review_rejected
      expect(Document.needs_review).not_to include(doc)
    end
  end

  describe "POST /documents/:id/reprocess" do
    it "resets both axes, clears the AI error, and re-enqueues processing" do
      # Reprocess is gated on an available AI provider; satisfy that precondition so
      # this example exercises the reset behavior, not the guard (cf. email_tools_spec).
      allow_any_instance_of(DocumentsController).to receive(:require_ai_provider!).and_return(false)
      doc = create(:document, :ai_failed, workspace: workspace)

      post reprocess_document_path(doc)

      doc.reload
      expect(doc).to be_ai_pending
      expect(doc).to be_review_pending
      expect(doc.ai_error).to be_nil
      expect(doc.ai_processing_attempts).to eq(0)
      expect(DocumentProcessJob).to have_received(:perform_later).with(doc.id)
    end
  end

  describe "isolation" do
    it "cannot act on another workspace's document" do
      other = create(:document, :in_review, workspace: create(:workspace))

      post approve_document_path(other)
      expect(response).to have_http_status(:not_found)
    end
  end
end
