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

  describe "PATCH /mail_folders/:id (rename)" do
    it "renames the folder and renames the provider folders" do
      folder = create(:mail_folder, workspace: workspace, name: "Receipts")
      expect(MailFolders::Provisioner).to receive(:rename_all).with(folder, "Receipts", user)
      patch mail_folder_path(folder), params: { mail_folder: { name: "Bills" } }, as: :turbo_stream
      expect(response).to have_http_status(:ok)
      expect(folder.reload.name).to eq("Bills")
    end

    it "does not call the provider rename when the name is unchanged" do
      folder = create(:mail_folder, workspace: workspace, name: "Receipts")
      expect(MailFolders::Provisioner).not_to receive(:rename_all)
      patch mail_folder_path(folder), params: { mail_folder: { icon: "star" } }, as: :turbo_stream
    end
  end

  describe "GET /mail_folders/:id (unified contents)" do
    it "renders the folder's documents and emails" do
      folder = create(:mail_folder, workspace: workspace, name: "Receipts")
      doc = create(:document, workspace: workspace)
      folder.documents << doc
      account = create(:email_account, workspace: workspace, provider: :zoho)
      create(:email_account_user, :viewer, user: user, email_account: account)
      create(:email_folder, email_account: account, name: "Receipts", provider_folder_id: "z-9")
      create(:email_message, email_account: account, provider_folder_id: "z-9", subject: "Hello invoice")

      get mail_folder_path(folder)

      expect(response).to have_http_status(:ok)
      expect(response.body).to include("/documents/#{doc.id}")
      expect(response.body).to include("Hello invoice")
    end

    it "404s for a folder in another workspace" do
      other = create(:mail_folder, workspace: create(:workspace), name: "Secret")
      get mail_folder_path(other)
      expect(response).to have_http_status(:not_found)
    end
  end

  # ── Dual-surface folder sync (pane + mobile sheet) — MailFoldersControllerTest ──
  #
  # Guards the dual-surface folder sync introduced with the mobile folder
  # bottom-sheet. create / update / destroy must re-render BOTH the desktop
  # pane's #pane_custom_folders AND the mobile sheet's #sheet_custom_folders,
  # so the two folder lists (both live in the DOM at once — the pane is CSS-hidden
  # on mobile, not removed) never drift out of sync.
  describe "dual-surface sync (pane + mobile sheet)" do
    include ActionView::RecordIdentifier

    # provision: false keeps the request hermetic (no provider API calls); with no
    # connected accounts provisioning is a no-op anyway, but this is explicit.
    it "create re-renders both the pane and the sheet custom-folder sections" do
      expect {
        post mail_folders_path,
             params: { mail_folder: { name: "Receipts" }, provision: false },
             as: :turbo_stream
      }.to change { workspace.mail_folders.count }.by(1)

      expect(response).to have_http_status(:ok)
      expect(response.body).to match(/target="custom_folder_chips"/)
      expect(response.body).to match(/target="pane_custom_folders"/)
      expect(response.body).to match(/target="sheet_custom_folders"/)
    end

    it "update re-renders both the pane and the sheet custom-folder sections" do
      folder = workspace.mail_folders.create!(name: "Clients", position: 1)

      patch mail_folder_path(folder),
            params: { mail_folder: { name: "Client Work" } },
            as: :turbo_stream

      expect(response).to have_http_status(:ok)
      expect(folder.reload.name).to eq("Client Work")
      expect(response.body).to match(/target="pane_custom_folders"/)
      expect(response.body).to match(/target="sheet_custom_folders"/)
    end

    it "destroy removes the chip and re-renders both custom-folder sections" do
      folder = workspace.mail_folders.create!(name: "Travel", position: 1)

      expect {
        delete mail_folder_path(folder), as: :turbo_stream
      }.to change { workspace.mail_folders.count }.by(-1)

      expect(response).to have_http_status(:ok)
      expect(response.body).to match(/target="#{dom_id(folder, :folder_chip)}"/)
      expect(response.body).to match(/target="pane_custom_folders"/)
      expect(response.body).to match(/target="sheet_custom_folders"/)
    end

    it "create requires authentication" do
      delete session_path
      post mail_folders_path,
           params: { mail_folder: { name: "Nope" }, provision: false },
           as: :turbo_stream

      expect(response).to have_http_status(:found)
    end
  end
end
