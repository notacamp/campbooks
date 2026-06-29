require "rails_helper"

RSpec.describe "Folder memberships (polymorphic filing)", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace) }
  let(:folder)    { create(:mail_folder, workspace: workspace) }
  before { sign_in(user) }

  def readable_email
    account = create(:email_account, workspace: workspace)
    create(:email_account_user, email_account: account, user: user, can_read: true)
    create(:email_message, email_account: account)
  end

  describe "POST /folder_memberships" do
    it "files an internal document and publishes file.filed" do
      doc = create(:authored_document, workspace: workspace)
      expect do
        post folder_memberships_path, params: {
          mail_folder_id: folder.id, folderable_id: doc.id, folderable_type: "AuthoredDocument"
        }
      end.to change { folder.authored_documents.count }.by(1)
      expect(workspace.events.where(name: "file.filed").count).to eq(1)
    end

    it "files a readable email and publishes email.filed" do
      email = readable_email
      expect do
        post folder_memberships_path, params: {
          mail_folder_id: folder.id, folderable_id: email.id, folderable_type: "EmailMessage"
        }
      end.to change { folder.email_messages.count }.by(1)
      expect(workspace.events.where(name: "email.filed").count).to eq(1)
    end

    it "404s for an email the user cannot read" do
      other = create(:email_message, email_account: create(:email_account, workspace: create(:workspace)))
      post folder_memberships_path, params: {
        mail_folder_id: folder.id, folderable_id: other.id, folderable_type: "EmailMessage"
      }
      expect(response).to have_http_status(:not_found)
    end

    it "404s for a disallowed folderable type" do
      post folder_memberships_path, params: {
        mail_folder_id: folder.id, folderable_id: user.id, folderable_type: "User"
      }
      expect(response).to have_http_status(:not_found)
    end

    it "404s for a document in another workspace" do
      other = create(:document, :other, workspace: create(:workspace))
      post folder_memberships_path, params: {
        mail_folder_id: folder.id, folderable_id: other.id, folderable_type: "Document"
      }
      expect(response).to have_http_status(:not_found)
    end
  end

  describe "DELETE /folder_memberships/:id" do
    it "removes an internal document from a folder and publishes file.unfiled" do
      doc = create(:authored_document, workspace: workspace)
      membership = folder.folder_memberships.create!(folderable: doc)
      expect do
        delete folder_membership_path(membership)
      end.to change { folder.authored_documents.count }.by(-1)
      expect(workspace.events.where(name: "file.unfiled").count).to eq(1)
    end
  end
end
