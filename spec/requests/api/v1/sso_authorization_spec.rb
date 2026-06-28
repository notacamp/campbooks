require "rails_helper"

# Browser SSO for the public API: the authorization_code + PKCE grant used by the
# Campbooks CLI. A public client, the signed-in app user as resource owner, a
# loopback redirect, and a short-lived bearer + refresh token.
RSpec.describe "API v1 browser SSO (authorization_code + PKCE)", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }
  let(:application) { create(:api_application, :public_client, scopes: "emails:read") }

  # A real loopback callback (with a port) — exercises Doorkeeper's RFC 8252
  # port-agnostic matching against the registered http://127.0.0.1/callback.
  let(:redirect_uri) { "http://127.0.0.1:51234/callback" }
  let(:code_verifier) { SecureRandom.urlsafe_base64(64) }
  let(:code_challenge) { Base64.urlsafe_encode64(Digest::SHA256.digest(code_verifier), padding: false) }

  def authorize_params(overrides = {})
    {
      client_id: application.uid,
      redirect_uri: redirect_uri,
      response_type: "code",
      scope: "emails:read",
      code_challenge: code_challenge,
      code_challenge_method: "S256",
      state: "the-state"
    }.merge(overrides)
  end

  # Approve the consent screen and pull the authorization code off the redirect.
  def obtain_code
    post oauth_authorization_path, params: authorize_params
    expect(response).to have_http_status(:found)
    location = response.headers["Location"]
    expect(location).to start_with(redirect_uri)
    query = Rack::Utils.parse_query(URI(location).query)
    expect(query["state"]).to eq("the-state")
    query["code"]
  end

  describe "GET /api/oauth/authorize" do
    it "renders the consent screen (scopes + client) when signed in" do
      sign_in(user)

      get oauth_authorization_path, params: authorize_params

      expect(response).to have_http_status(:ok)
      expect(response.body).to include(application.name)
      expect(response.body).to include(Api::Scopes.description("emails:read"))
    end

    it "redirects to sign-in when not authenticated" do
      get oauth_authorization_path, params: authorize_params

      expect(response).to redirect_to("/session/new")
    end
  end

  describe "the full PKCE code exchange" do
    before { sign_in(user) }

    it "issues a resource-owner token that acts as the signed-in user" do
      code = obtain_code

      post oauth_token_path, params: {
        grant_type: "authorization_code",
        code: code,
        client_id: application.uid,
        redirect_uri: redirect_uri,
        code_verifier: code_verifier
      }

      expect(response).to have_http_status(:ok)
      body = response.parsed_body
      expect(body["access_token"]).to be_present
      expect(body["refresh_token"]).to be_present
      expect(body["scope"]).to eq("emails:read")

      # The token is bridged to the signed-in user + their workspace.
      get api_v1_emails_path, headers: { "Authorization" => "Bearer #{body['access_token']}" }
      expect(response).to have_http_status(:ok)
    end

    it "rejects the exchange when the PKCE verifier doesn't match" do
      code = obtain_code

      post oauth_token_path, params: {
        grant_type: "authorization_code",
        code: code,
        client_id: application.uid,
        redirect_uri: redirect_uri,
        code_verifier: "the-wrong-verifier-#{SecureRandom.hex(16)}"
      }

      expect(response).to have_http_status(:bad_request)
      expect(response.parsed_body.dig("error", "code")).to eq("invalid_grant")
    end

    it "refreshes silently with the refresh token" do
      code = obtain_code
      first = response_for_token_exchange(code)
      refresh_token = first["refresh_token"]

      post oauth_token_path, params: {
        grant_type: "refresh_token",
        refresh_token: refresh_token,
        client_id: application.uid
      }

      expect(response).to have_http_status(:ok)
      expect(response.parsed_body["access_token"]).to be_present
      expect(response.parsed_body["access_token"]).not_to eq(first["access_token"])
    end
  end

  describe "PKCE enforcement (public client)" do
    before { sign_in(user) }

    it "never issues a code without PKCE" do
      post oauth_authorization_path, params: authorize_params.except(:code_challenge, :code_challenge_method)

      # force_pkce: a public client cannot get a code without PKCE. raise-mode +
      # our rescue render the friendly error page (400); no code is ever granted.
      expect(response).to have_http_status(:bad_request)
      expect(response.body).not_to include("code=")
      expect(response.headers["Location"].to_s).not_to include("code=")
    end
  end

  describe "establish_acting_identity for a revoked resource owner" do
    it "401s (client_revoked) when the token's user has been deleted" do
      token = create(:api_access_token, application: application,
                                        resource_owner_id: user.id, scopes: "emails:read")
      user.destroy

      get api_v1_emails_path, headers: { "Authorization" => "Bearer #{token.plaintext_token}" }

      expect(response).to have_http_status(:unauthorized)
      expect(response.parsed_body.dig("error", "code")).to eq("client_revoked")
    end
  end

  def response_for_token_exchange(code)
    post oauth_token_path, params: {
      grant_type: "authorization_code",
      code: code,
      client_id: application.uid,
      redirect_uri: redirect_uri,
      code_verifier: code_verifier
    }
    response.parsed_body
  end
end
