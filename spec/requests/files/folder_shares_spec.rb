require "rails_helper"

RSpec.describe "Files::FolderShares", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace) }
  let(:other)     { create(:user, workspace: workspace) }
  before { sign_in(user) }

  describe "PATCH /files/folders/:folder_id/share" do
    it "restricts an open folder and makes the toggler its owner" do
      folder = create(:mail_folder, workspace: workspace)

      patch files_folder_share_path(folder), params: { restricted: "true" }

      expect(folder.reload.restricted?).to be(true)
      expect(folder.mail_folder_users.find_by(user: user)).to have_attributes(owner: true)
      expect(workspace.events.where(name: "folder.restricted").count).to eq(1)
    end

    it "adds a member with a role and publishes folder.shared" do
      folder = create(:mail_folder, workspace: workspace, restricted: true)
      folder.mail_folder_users.create!(user: user, owner: true, can_read: true, can_write: true, can_manage: true)

      expect do
        patch files_folder_share_path(folder), params: { user_email: other.email_address, role: "editor" }
      end.to change { folder.mail_folder_users.count }.by(1)

      expect(folder.mail_folder_users.find_by(user: other).role).to eq("editor")
      expect(workspace.events.where(name: "folder.shared").count).to eq(1)
    end

    it "removes a member" do
      folder = create(:mail_folder, workspace: workspace, restricted: true)
      folder.mail_folder_users.create!(user: user, owner: true, can_manage: true)
      folder.mail_folder_users.create!(user: other, can_read: true)

      expect do
        patch files_folder_share_path(folder), params: { user_email: other.email_address, remove: "true" }
      end.to change { folder.mail_folder_users.count }.by(-1)
    end

    it "404s for a user who cannot manage a restricted folder" do
      folder = create(:mail_folder, workspace: workspace, restricted: true)
      folder.mail_folder_users.create!(user: other, owner: true, can_manage: true)

      patch files_folder_share_path(folder), params: { restricted: "false" }

      expect(response).to have_http_status(:not_found)
      expect(folder.reload.restricted?).to be(true)
    end
  end

  describe "GET /files/folders/:folder_id/share" do
    it "renders the panel for someone who can manage the folder" do
      folder = create(:mail_folder, workspace: workspace) # open → any member may manage
      get files_folder_share_path(folder)
      expect(response).to have_http_status(:ok)
    end
  end
end
