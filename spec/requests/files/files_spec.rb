require "rails_helper"

RSpec.describe "Files", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace) }
  before { sign_in(user) }

  describe "GET /files" do
    it "lists the workspace's files" do
      doc = create(:document, :other, workspace: workspace)
      get files_path
      expect(response).to have_http_status(:ok)
      expect(response.body).to include(doc.display_title)
    end

    it "renders the empty state when there are none" do
      get files_path
      expect(response).to have_http_status(:ok)
      expect(response.body).to include(I18n.t("files.index.empty_title"))
    end
  end

  describe "GET /files/folders/:id" do
    it "shows a folder and the files filed into it" do
      folder = create(:mail_folder, workspace: workspace, name: "Contracts")
      doc = create(:document, :other, workspace: workspace)
      folder.folder_memberships.create!(folderable: doc)

      get files_folder_path(folder)

      expect(response).to have_http_status(:ok)
      expect(response.body).to include("Contracts")
      expect(response.body).to include(doc.display_title)
    end

    it "404s for a folder in another workspace" do
      other = create(:mail_folder, workspace: create(:workspace))
      get files_folder_path(other)
      expect(response).to have_http_status(:not_found)
    end
  end
end
