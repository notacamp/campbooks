require "rails_helper"

RSpec.describe "Public file links", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace) }

  describe "POST /files/public_links (authenticated)" do
    before { sign_in(user) }

    it "mints a link for an accessible document and returns the URL" do
      doc = create(:document, :other, workspace: workspace)
      expect do
        post files_public_links_path, params: { shareable_type: "Document", shareable_id: doc.id }, as: :json
      end.to change(FileShareLink, :count).by(1)
      expect(response.parsed_body["url"]).to be_present
      expect(workspace.events.where(name: "file.made_public").count).to eq(1)
    end

    it "reuses an existing active link instead of minting a second" do
      doc = create(:document, :other, workspace: workspace)
      FileShareLink.create!(shareable: doc, workspace: workspace, created_by: user)
      expect do
        post files_public_links_path, params: { shareable_type: "Document", shareable_id: doc.id }, as: :json
      end.not_to change(FileShareLink, :count)
    end

    it "404s for a document in another workspace" do
      other = create(:document, :other, workspace: create(:workspace))
      post files_public_links_path, params: { shareable_type: "Document", shareable_id: other.id }, as: :json
      expect(response).to have_http_status(:not_found)
    end

    it "revokes a link" do
      doc = create(:document, :other, workspace: workspace)
      link = FileShareLink.create!(shareable: doc, workspace: workspace, created_by: user)
      delete files_public_link_path(link)
      expect(link.reload.revoked?).to be(true)
    end
  end

  describe "GET /f/:token (public, no auth)" do
    it "redirects to the file for a live link and counts the view" do
      doc = create(:document, :other, workspace: workspace)
      link = FileShareLink.create!(shareable: doc, workspace: workspace)
      expect do
        get public_file_path(token: link.token)
      end.to change { link.reload.view_count }.by(1)
      expect(response).to have_http_status(:redirect)
    end

    it "404s for a revoked link" do
      doc = create(:document, :other, workspace: workspace)
      link = FileShareLink.create!(shareable: doc, workspace: workspace, revoked_at: Time.current)
      get public_file_path(token: link.token)
      expect(response).to have_http_status(:not_found)
    end

    it "renders an internal document publicly" do
      authored = create(:authored_document, workspace: workspace, title: "Public brief", html_content: "<p>Visible body</p>")
      link = FileShareLink.create!(shareable: authored, workspace: workspace)
      get public_file_path(token: link.token)
      expect(response).to have_http_status(:ok)
      expect(response.body).to include("Public brief")
      expect(response.body).to include("Visible body")
    end
  end
end
