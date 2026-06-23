require "rails_helper"

RSpec.describe "API v1 contacts", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }

  def read_headers
    api_auth_headers(workspace: workspace, user: user, scopes: "contacts:read")
  end

  def write_headers
    api_auth_headers(workspace: workspace, user: user, scopes: "contacts:write")
  end

  describe "GET /api/v1/contacts" do
    it "lists workspace contacts and filters by starred" do
      create(:contact, workspace: workspace, email: "a@acme.com", name: "Acme")
      create(:contact, workspace: workspace, email: "b@other.com", starred_at: Time.current)

      get api_v1_contacts_path, params: { starred: true }, headers: read_headers

      expect(response).to have_http_status(:ok)
      data = response.parsed_body["data"]
      expect(data.size).to eq(1)
      expect(data.first["starred"]).to be(true)
    end

    it "does not leak another workspace's contacts" do
      create(:contact, workspace: create(:workspace))

      get api_v1_contacts_path, headers: read_headers

      expect(response.parsed_body["data"]).to be_empty
    end
  end

  describe "GET /api/v1/contacts/:id" do
    it "404s across workspaces" do
      contact = create(:contact, workspace: create(:workspace))

      get api_v1_contact_path(contact), headers: read_headers

      expect(response).to have_http_status(:not_found)
    end
  end

  describe "PATCH /api/v1/contacts/:id" do
    it "updates name + relationship through the linked person" do
      contact = create(:contact, workspace: workspace)

      patch api_v1_contact_path(contact),
            params: { name: "New Name", relationship_type: "client" }, headers: write_headers

      expect(response).to have_http_status(:ok)
      expect(contact.reload.name).to eq("New Name")
      expect(contact.relationship_type).to eq("client")
      expect(contact.person).to be_present
    end

    it "403s with only the read scope" do
      contact = create(:contact, workspace: workspace)

      patch api_v1_contact_path(contact), params: { name: "X" }, headers: read_headers

      expect(response).to have_http_status(:forbidden)
    end
  end

  describe "POST /api/v1/contacts/:id/state" do
    it "stars the contact" do
      contact = create(:contact, workspace: workspace)

      post state_api_v1_contact_path(contact), params: { state: "star" }, headers: write_headers

      expect(response).to have_http_status(:ok)
      expect(contact.reload.starred?).to be(true)
    end

    it "blocks via the Contacts::Block service (acting as the API user)" do
      contact = create(:contact, workspace: workspace)
      allow(Contacts::Block).to receive(:call)

      post state_api_v1_contact_path(contact), params: { state: "block" }, headers: write_headers

      expect(response).to have_http_status(:ok)
      expect(Contacts::Block).to have_received(:call).with(have_attributes(id: contact.id), user: user)
    end

    it "422s for an invalid state" do
      contact = create(:contact, workspace: workspace)

      post state_api_v1_contact_path(contact), params: { state: "bogus" }, headers: write_headers

      expect(response).to have_http_status(:unprocessable_entity)
    end
  end
end
