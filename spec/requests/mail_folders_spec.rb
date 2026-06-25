require "rails_helper"

RSpec.describe "MailFolders", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }
  before { sign_in(user) }

  describe "PATCH /mail_folders/:id" do
    let(:folder) { create(:mail_folder, workspace: workspace, name: "Receipts", icon: "folder") }

    it "updates the icon and refreshes the pane section" do
      patch mail_folder_path(folder), params: { mail_folder: { icon: "star" } }, as: :turbo_stream
      expect(response).to have_http_status(:ok)
      expect(folder.reload.icon).to eq("star")
      expect(response.body).to include("pane_custom_folders")
    end

    it "rejects an unknown icon" do
      patch mail_folder_path(folder), params: { mail_folder: { icon: "definitely-not-an-icon" } }, as: :turbo_stream
      expect(response).to have_http_status(:unprocessable_entity)
      expect(folder.reload.icon).to eq("folder")
    end
  end

  describe "PATCH /mail_folders/:id (move)" do
    it "moves a folder under a parent" do
      parent = create(:mail_folder, workspace: workspace, name: "Work")
      folder = create(:mail_folder, workspace: workspace, name: "Clients")
      patch mail_folder_path(folder), params: { mail_folder: { parent_id: parent.id } }, as: :turbo_stream
      expect(response).to have_http_status(:ok)
      expect(folder.reload.parent_id).to eq(parent.id)
    end

    it "moves a folder back to top level" do
      parent = create(:mail_folder, workspace: workspace, name: "Work")
      folder = create(:mail_folder, workspace: workspace, name: "Clients", parent: parent)
      patch mail_folder_path(folder), params: { mail_folder: { parent_id: "" } }, as: :turbo_stream
      expect(folder.reload.parent_id).to be_nil
    end

    it "rejects a cyclic move" do
      a = create(:mail_folder, workspace: workspace, name: "A")
      b = create(:mail_folder, workspace: workspace, name: "B", parent: a)
      patch mail_folder_path(a), params: { mail_folder: { parent_id: b.id } }, as: :turbo_stream
      expect(response).to have_http_status(:unprocessable_entity)
      expect(a.reload.parent_id).to be_nil
    end
  end

  describe "DELETE /mail_folders/:id" do
    it "removes the folder and refreshes the pane section" do
      folder = create(:mail_folder, workspace: workspace, name: "Receipts")
      expect { delete mail_folder_path(folder), as: :turbo_stream }.to change(MailFolder, :count).by(-1)
      expect(response.body).to include("pane_custom_folders")
    end
  end
end
