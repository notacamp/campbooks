require "rails_helper"

RSpec.describe "API v1 folders", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }

  def read_headers
    api_auth_headers(workspace: workspace, user: user, scopes: "folders:read")
  end

  def write_headers
    api_auth_headers(workspace: workspace, user: user, scopes: "folders:write")
  end

  describe "GET /api/v1/folders" do
    it "lists workspace folders in order" do
      create(:mail_folder, workspace: workspace, name: "Alpha", position: 0)
      create(:mail_folder, workspace: workspace, name: "Beta", position: 1)

      get api_v1_folders_path, headers: read_headers

      expect(response).to have_http_status(:ok)
      names = response.parsed_body["data"].map { |f| f["name"] }
      expect(names).to include("Alpha", "Beta")
    end

    it "does not leak another workspace's folders" do
      create(:mail_folder, workspace: create(:workspace))

      get api_v1_folders_path, headers: read_headers

      expect(response.parsed_body["data"]).to be_empty
    end

    it "403s without the folders:read scope" do
      get api_v1_folders_path,
          headers: api_auth_headers(workspace: workspace, user: user, scopes: "emails:read")

      expect(response).to have_http_status(:forbidden)
    end
  end

  describe "GET /api/v1/folders/:id" do
    it "returns the folder with its documents" do
      folder   = create(:mail_folder, workspace: workspace)
      document = create(:document, workspace: workspace)
      folder.folder_memberships.create!(folderable: document)

      get api_v1_folder_path(folder), headers: read_headers

      expect(response).to have_http_status(:ok)
      body = response.parsed_body["data"]
      expect(body["id"]).to eq(folder.id)
      expect(body["documents"]).to be_an(Array)
      expect(body["documents"].first["id"]).to eq(document.id)
    end

    it "404s for a folder in another workspace" do
      other_folder = create(:mail_folder, workspace: create(:workspace))

      get api_v1_folder_path(other_folder), headers: read_headers

      expect(response).to have_http_status(:not_found)
    end
  end

  describe "POST /api/v1/folder_memberships" do
    it "files a document into a folder and returns 201" do
      folder   = create(:mail_folder, workspace: workspace)
      document = create(:document, workspace: workspace)

      post api_v1_folder_memberships_path,
           params: { mail_folder_id: folder.id, document_id: document.id },
           headers: write_headers

      expect(response).to have_http_status(:created)
      body = response.parsed_body["data"]
      expect(body["folder_id"]).to eq(folder.id)
      expect(body["document_id"]).to eq(document.id)
      expect(FolderMembership.count).to eq(1)
    end

    it "is idempotent — duplicate request returns 201 without creating a second membership" do
      folder   = create(:mail_folder, workspace: workspace)
      document = create(:document, workspace: workspace)
      folder.folder_memberships.create!(folderable: document)

      post api_v1_folder_memberships_path,
           params: { mail_folder_id: folder.id, document_id: document.id },
           headers: write_headers

      expect(response).to have_http_status(:created)
      expect(FolderMembership.count).to eq(1)
    end

    it "404s when the document belongs to another workspace" do
      folder   = create(:mail_folder, workspace: workspace)
      document = create(:document, workspace: create(:workspace))

      post api_v1_folder_memberships_path,
           params: { mail_folder_id: folder.id, document_id: document.id },
           headers: write_headers

      expect(response).to have_http_status(:not_found)
    end

    it "403s with only the folders:read scope" do
      folder   = create(:mail_folder, workspace: workspace)
      document = create(:document, workspace: workspace)

      post api_v1_folder_memberships_path,
           params: { mail_folder_id: folder.id, document_id: document.id },
           headers: read_headers

      expect(response).to have_http_status(:forbidden)
    end
  end

  describe "DELETE /api/v1/folder_memberships/:id" do
    it "removes the membership and returns 204" do
      folder     = create(:mail_folder, workspace: workspace)
      document   = create(:document, workspace: workspace)
      membership = folder.folder_memberships.create!(folderable: document)

      delete api_v1_folder_membership_path(membership), headers: write_headers

      expect(response).to have_http_status(:no_content)
      expect(FolderMembership.exists?(membership.id)).to be(false)
    end

    it "404s for a membership in another workspace" do
      other_folder = create(:mail_folder, workspace: create(:workspace))
      document     = create(:document, workspace: create(:workspace))
      membership   = other_folder.folder_memberships.create!(folderable: document)

      delete api_v1_folder_membership_path(membership), headers: write_headers

      expect(response).to have_http_status(:not_found)
    end
  end
end
