require "rails_helper"

RSpec.describe "API v1 documents", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }

  def read_headers
    api_auth_headers(workspace: workspace, user: user, scopes: "documents:read")
  end

  describe "GET /api/v1/documents" do
    it "lists workspace documents" do
      create(:document, workspace: workspace)

      get api_v1_documents_path, headers: read_headers

      expect(response).to have_http_status(:ok)
      expect(response.parsed_body["data"].size).to eq(1)
    end

    it "does not leak documents from another workspace" do
      create(:document, workspace: create(:workspace))

      get api_v1_documents_path, headers: read_headers

      expect(response.parsed_body["data"]).to be_empty
    end
  end

  describe "GET /api/v1/documents/:id" do
    it "404s for a document in another workspace" do
      document = create(:document, workspace: create(:workspace))

      get api_v1_document_path(document), headers: read_headers

      expect(response).to have_http_status(:not_found)
    end
  end

  describe "POST /api/v1/documents (upload)" do
    it "creates documents, enqueues processing, and returns 202" do
      allow(DocumentProcessJob).to receive(:perform_later)
      headers = api_auth_headers(workspace: workspace, user: user, scopes: "documents:write")
      file = Rack::Test::UploadedFile.new(StringIO.new("%PDF-1.4 fake"), "application/pdf",
                                          original_filename: "invoice.pdf")

      expect {
        post api_v1_documents_path, params: { files: [ file ] }, headers: headers
      }.to change(Document, :count).by(1)

      expect(response).to have_http_status(:accepted)
      expect(response.parsed_body["data"].first["ai_status"]).to eq("pending")
      expect(DocumentProcessJob).to have_received(:perform_later)
    end

    it "400s when no files are given" do
      headers = api_auth_headers(workspace: workspace, user: user, scopes: "documents:write")

      post api_v1_documents_path, params: {}, headers: headers

      expect(response).to have_http_status(:bad_request)
    end

    it "403s with only the read scope" do
      file = Rack::Test::UploadedFile.new(StringIO.new("x"), "application/pdf", original_filename: "x.pdf")

      post api_v1_documents_path, params: { files: [ file ] }, headers: read_headers

      expect(response).to have_http_status(:forbidden)
    end
  end
end
