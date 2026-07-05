# frozen_string_literal: true

require "rails_helper"

RSpec.describe "API MCP key authentication", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace) }

  # Build an MCP request with a raw Authorization header (no token minting).
  # Note: `app` is NOT used as a let name here — it shadows ActionDispatch's
  # integration-session `app` method (the Rack stack) and breaks request dispatch.
  def mcp_key_request(rpc_method, rpc_params = {}, auth_header:)
    post "/api/mcp",
         params: { jsonrpc: "2.0", id: 1, method: rpc_method, params: rpc_params }.to_json,
         headers: { "Authorization" => auth_header, "CONTENT_TYPE" => "application/json" }
  end

  # Convenience: tools/list via a uid.secret Bearer MCP key.
  def tools_list_with_key(uid:, secret:)
    mcp_key_request("tools/list", {}, auth_header: "Bearer #{uid}.#{secret}")
  end

  describe "Bearer uid.secret (MCP key form)" do
    context "with a valid confidential application" do
      let!(:client) { create(:api_application, workspace: workspace, created_by: user, scopes: "emails:read tags:read") }
      let(:secret)  { client.plaintext_secret }

      it "authenticates and returns a tools/list scoped to the application's scopes" do
        tools_list_with_key(uid: client.uid, secret: secret)

        expect(response).to have_http_status(:ok)
        names = response.parsed_body.dig("result", "tools").map { |t| t["name"] }
        expect(names).to include("list_emails", "list_tags")
        # emails:send not granted — send_email must be absent
        expect(names).not_to include("send_email")
      end

      it "seeds Current.api_scopes from the application's own scopes" do
        # Verify indirectly: non-empty tools list means granted_scope_names returned
        # the application's scopes correctly.
        tools_list_with_key(uid: client.uid, secret: secret)

        names = response.parsed_body.dig("result", "tools").map { |t| t["name"] }
        expect(names.size).to be > 0
      end
    end

    context "tools/call under key auth" do
      let!(:client) { create(:api_application, workspace: workspace, created_by: user, scopes: "tags:read") }
      let(:secret)  { client.plaintext_secret }

      before { Tag.create!(workspace: workspace, name: "Invoice", color: "#abc", source: :local) }

      it "runs a tool and returns the result" do
        mcp_key_request("tools/call", { name: "list_tags", arguments: {} },
                        auth_header: "Bearer #{client.uid}.#{secret}")

        expect(response).to have_http_status(:ok)
        payload = JSON.parse(response.parsed_body.dig("result", "content", 0, "text"))
        expect(payload["tags"].map { |t| t["name"] }).to include("Invoice")
      end
    end

    context "error cases" do
      let!(:client) { create(:api_application, workspace: workspace, created_by: user, scopes: "emails:read") }
      let(:secret)  { client.plaintext_secret }

      it "returns 401 invalid_client when the secret is wrong" do
        tools_list_with_key(uid: client.uid, secret: "wrongsecret")

        expect(response).to have_http_status(:unauthorized)
        expect(response.parsed_body.dig("error", "code")).to eq("invalid_client")
      end

      it "returns 401 invalid_client when the uid is unknown" do
        tools_list_with_key(uid: "nosuchuid", secret: secret)

        expect(response).to have_http_status(:unauthorized)
        expect(response.parsed_body.dig("error", "code")).to eq("invalid_client")
      end

      it "returns 403 insufficient_scope when the application has no scopes" do
        client.update_column(:scopes, "")

        tools_list_with_key(uid: client.uid, secret: secret)

        expect(response).to have_http_status(:forbidden)
        expect(response.parsed_body.dig("error", "code")).to eq("insufficient_scope")
      end

      it "returns 401 when the application is not confidential" do
        public_client = create(:api_application, :public_client)
        tools_list_with_key(uid: public_client.uid, secret: "any")

        expect(response).to have_http_status(:unauthorized)
        expect(response.parsed_body.dig("error", "code")).to eq("invalid_client")
      end

      it "returns 401 client_revoked when the application's created_by user is gone" do
        # Null out created_by_id to simulate the user having been deleted.
        # (A hard destroy! fails on the FK constraint; nulling the reference is
        # equivalent from establish_acting_identity!'s perspective — acting_user
        # resolves to nil and the check fails closed with client_revoked.)
        client.update_column(:created_by_id, nil)

        tools_list_with_key(uid: client.uid, secret: secret)

        expect(response).to have_http_status(:unauthorized)
        expect(response.parsed_body.dig("error", "code")).to eq("client_revoked")
      end
    end
  end

  describe "Basic auth (uid:secret in base64)" do
    let!(:client)      { create(:api_application, workspace: workspace, created_by: user, scopes: "emails:read") }
    let(:secret)       { client.plaintext_secret }
    let(:basic_header) { "Basic #{Base64.strict_encode64("#{client.uid}:#{secret}")}" }

    it "authenticates and returns tools/list" do
      mcp_key_request("tools/list", {}, auth_header: basic_header)

      expect(response).to have_http_status(:ok)
      names = response.parsed_body.dig("result", "tools").map { |t| t["name"] }
      expect(names).to include("list_emails")
    end

    it "returns 401 with wrong secret in Basic credentials" do
      bad_header = "Basic #{Base64.strict_encode64("#{client.uid}:wrongsecret")}"
      mcp_key_request("tools/list", {}, auth_header: bad_header)

      expect(response).to have_http_status(:unauthorized)
      expect(response.parsed_body.dig("error", "code")).to eq("invalid_client")
    end
  end

  describe "plain Doorkeeper bearer token (unchanged path)" do
    it "still authenticates with a short-lived token" do
      headers = api_auth_headers(workspace: workspace, user: user, scopes: "emails:read")
                .merge("CONTENT_TYPE" => "application/json")
      post "/api/mcp",
           params: { jsonrpc: "2.0", id: 1, method: "tools/list" }.to_json,
           headers: headers

      expect(response).to have_http_status(:ok)
      names = response.parsed_body.dig("result", "tools").map { |t| t["name"] }
      expect(names).to include("list_emails")
    end

    it "still returns 401 with a missing token" do
      post "/api/mcp",
           params: { jsonrpc: "2.0", id: 1, method: "tools/list" }.to_json,
           headers: { "CONTENT_TYPE" => "application/json" }

      expect(response).to have_http_status(:unauthorized)
    end
  end

  describe "scope filtering under key auth" do
    let!(:client) { create(:api_application, workspace: workspace, created_by: user, scopes: "tags:read") }
    let(:secret)  { client.plaintext_secret }

    it "excludes tools the application's scopes do not cover" do
      tools_list_with_key(uid: client.uid, secret: secret)

      names = response.parsed_body.dig("result", "tools").map { |t| t["name"] }
      expect(names).to include("list_tags")
      expect(names).not_to include("list_emails", "send_email", "list_documents")
    end
  end
end
