require "rails_helper"

RSpec.describe "API v1 authentication", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }

  describe "POST /api/oauth/token (client_credentials)" do
    it "issues a bearer token for valid client credentials" do
      application = create(:api_application, workspace: workspace, created_by: user, scopes: "emails:read")

      post oauth_token_path, params: {
        grant_type: "client_credentials",
        client_id: application.uid,
        client_secret: application.plaintext_secret,
        scope: "emails:read"
      }

      expect(response).to have_http_status(:ok)
      body = response.parsed_body
      expect(body["access_token"]).to be_present
      expect(body["token_type"]).to eq("Bearer")
      expect(body["scope"]).to eq("emails:read")
    end

    it "rejects bad credentials with the API error envelope" do
      post oauth_token_path, params: {
        grant_type: "client_credentials",
        client_id: "nope",
        client_secret: "wrong"
      }

      expect(response).to have_http_status(:unauthorized)
      expect(response.parsed_body.dig("error", "code")).to eq("invalid_client")
    end
  end

  describe "bearer-token protection on /api/v1/*" do
    it "401s without a token" do
      get api_v1_emails_path

      expect(response).to have_http_status(:unauthorized)
      expect(response.parsed_body.dig("error", "code")).to eq("invalid_token")
    end

    it "401s with a bogus token" do
      get api_v1_emails_path, headers: { "Authorization" => "Bearer not-a-real-token" }

      expect(response).to have_http_status(:unauthorized)
    end

    it "403s when the token lacks the required scope" do
      headers = api_auth_headers(workspace: workspace, user: user, scopes: "documents:read")

      get api_v1_emails_path, headers: headers

      expect(response).to have_http_status(:forbidden)
      expect(response.parsed_body.dig("error", "code")).to eq("insufficient_scope")
    end

    it "403s when the token carries no scopes at all" do
      application = create(:api_application, workspace: workspace, created_by: user, scopes: "emails:read")
      token = create(:api_access_token, application: application, scopes: "")

      get api_v1_emails_path, headers: api_headers(token)

      expect(response).to have_http_status(:forbidden)
    end

    it "401s (client_revoked) when the client's acting user no longer matches the workspace" do
      application = create(:api_application, workspace: workspace, created_by: user, scopes: "emails:read")
      token = create(:api_access_token, application: application, scopes: "emails:read")
      # Simulate the acting user being reassigned/removed from the workspace.
      user.update_columns(workspace_id: create(:workspace).id)

      get api_v1_emails_path, headers: api_headers(token)

      expect(response).to have_http_status(:unauthorized)
      expect(response.parsed_body.dig("error", "code")).to eq("client_revoked")
    end
  end
end
