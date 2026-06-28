require "rails_helper"

RSpec.describe "Documents::Written", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace) }
  before { sign_in(user) }

  describe "GET /documents/write" do
    it "lists the workspace's authored documents" do
      doc = create(:authored_document, workspace: workspace, title: "Quarterly report")
      get written_documents_path
      expect(response).to have_http_status(:ok)
      expect(response.body).to include("Quarterly report")
    end

    it "renders the empty state when there are none" do
      get written_documents_path
      expect(response).to have_http_status(:ok)
    end
  end

  describe "GET /documents/write/new" do
    it "renders the editor" do
      get new_written_document_path
      expect(response).to have_http_status(:ok)
    end
  end

  describe "POST /documents/write" do
    it "creates a document scoped to the workspace and author, then redirects to it" do
      expect do
        post written_documents_path, params: { authored_document: { title: "My doc", html_content: "<p>Hi</p>" } }
      end.to change(workspace.authored_documents, :count).by(1)

      doc = workspace.authored_documents.last
      expect(doc.title).to eq("My doc")
      expect(doc.author).to eq(user)
      expect(response).to redirect_to(written_document_path(doc))
    end

    it "re-renders with 422 when the title is blank" do
      post written_documents_path, params: { authored_document: { title: "", html_content: "<p>Hi</p>" } }
      expect(response).to have_http_status(:unprocessable_entity)
    end
  end

  describe "GET /documents/write/:id" do
    it "renders the sanitized document" do
      doc = create(:authored_document, workspace: workspace, html_content: "<p>Body text</p>")
      get written_document_path(doc)
      expect(response).to have_http_status(:ok)
      expect(response.body).to include("Body text")
    end
  end

  describe "GET /documents/write/:id/edit" do
    it "renders the editor pre-filled" do
      doc = create(:authored_document, workspace: workspace)
      get edit_written_document_path(doc)
      expect(response).to have_http_status(:ok)
    end
  end

  describe "PATCH /documents/write/:id" do
    it "updates the document and redirects to it" do
      doc = create(:authored_document, workspace: workspace, title: "Old")
      patch written_document_path(doc), params: { authored_document: { title: "New" } }
      expect(doc.reload.title).to eq("New")
      expect(response).to redirect_to(written_document_path(doc))
    end
  end

  describe "cross-workspace isolation" do
    it "404s for a document in another workspace" do
      other = create(:authored_document, workspace: create(:workspace))
      get written_document_path(other)
      expect(response).to have_http_status(:not_found)
    end
  end
end
