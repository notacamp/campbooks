require "rails_helper"

RSpec.describe "FolderMemberships", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }
  let(:folder) { create(:mail_folder, workspace: workspace) }
  let(:document) { create(:document, workspace: workspace) }
  before { sign_in(user) }

  describe "POST /folder_memberships" do
    it "files a document into a folder" do
      expect {
        post folder_memberships_path,
          params: { mail_folder_id: folder.id, folderable_id: document.id, folderable_type: "Document" }, as: :turbo_stream
      }.to change(FolderMembership, :count).by(1)
      expect(document.reload.mail_folders).to include(folder)
    end

    it "is idempotent" do
      folder.documents << document
      expect {
        post folder_memberships_path, params: { mail_folder_id: folder.id, folderable_id: document.id }, as: :turbo_stream
      }.not_to change(FolderMembership, :count)
    end

    it "404s for a document in another workspace (no leak)" do
      other = create(:document, workspace: create(:workspace))
      post folder_memberships_path, params: { mail_folder_id: folder.id, folderable_id: other.id }, as: :turbo_stream
      expect(response).to have_http_status(:not_found)
      expect(FolderMembership.count).to eq(0)
    end
  end

  describe "DELETE /folder_memberships/:id" do
    it "removes the document from the folder" do
      folder.documents << document
      membership = FolderMembership.last
      expect { delete folder_membership_path(membership), as: :turbo_stream }.to change(FolderMembership, :count).by(-1)
    end

    it "404s for a membership in another workspace" do
      other_folder = create(:mail_folder, workspace: create(:workspace))
      other_folder.documents << create(:document, workspace: other_folder.workspace)
      membership = FolderMembership.last
      delete folder_membership_path(membership), as: :turbo_stream
      expect(response).to have_http_status(:not_found)
      expect(FolderMembership.exists?(membership.id)).to be(true)
    end
  end
end
