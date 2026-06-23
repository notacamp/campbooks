require "rails_helper"

RSpec.describe "Settings::ApiClients", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }

  before { sign_in(user) }

  describe "GET /settings/api_clients" do
    it "lists this workspace's clients" do
      create(:api_application, workspace: workspace, created_by: user, name: "My Client")

      get settings_api_clients_path

      expect(response).to have_http_status(:ok)
      expect(response.body).to include("My Client")
    end

    it "does not show another workspace's clients" do
      create(:api_application, name: "Other Client")

      get settings_api_clients_path

      expect(response.body).not_to include("Other Client")
    end
  end

  describe "GET /settings/api_clients/new" do
    it "renders the form" do
      get new_settings_api_client_path

      expect(response).to have_http_status(:ok)
    end
  end

  describe "POST /settings/api_clients" do
    it "creates a workspace-scoped client and reveals the secret once" do
      expect {
        post settings_api_clients_path, params: { application: { name: "Billing", scopes: [ "emails:read" ] } }
      }.to change(Doorkeeper::Application, :count).by(1)

      application = Doorkeeper::Application.order(:created_at).last
      expect(application.workspace).to eq(workspace)
      expect(application.created_by).to eq(user)
      expect(application.scopes.to_a).to eq([ "emails:read" ])
      expect(response).to have_http_status(:created)
      expect(response.body).to include(application.uid)
    end

    it "drops unknown scopes and rejects a client with none" do
      expect {
        post settings_api_clients_path, params: { application: { name: "Bad", scopes: [ "bogus:scope" ] } }
      }.not_to change(Doorkeeper::Application, :count)

      expect(response).to have_http_status(:unprocessable_entity)
    end
  end

  describe "POST /settings/api_clients/:id/regenerate_secret" do
    it "rotates the stored secret" do
      application = create(:api_application, workspace: workspace, created_by: user)
      old_secret = application.secret

      post regenerate_secret_settings_api_client_path(application)

      expect(response).to have_http_status(:ok)
      expect(application.reload.secret).not_to eq(old_secret)
    end
  end

  describe "POST /settings/api_clients/:id/revoke" do
    it "revokes the client's active tokens" do
      application = create(:api_application, workspace: workspace, created_by: user)
      token = create(:api_access_token, application: application)

      post revoke_settings_api_client_path(application)

      expect(token.reload.revoked_at).to be_present
    end
  end

  describe "DELETE /settings/api_clients/:id" do
    it "deletes this workspace's client" do
      application = create(:api_application, workspace: workspace, created_by: user)

      expect {
        delete settings_api_client_path(application)
      }.to change(Doorkeeper::Application, :count).by(-1)
    end

    it "404s and leaves another workspace's client intact" do
      other = create(:api_application, name: "Other")

      delete settings_api_client_path(other)

      expect(response).to have_http_status(:not_found)
      expect(Doorkeeper::Application.exists?(other.id)).to be(true)
    end
  end
end
