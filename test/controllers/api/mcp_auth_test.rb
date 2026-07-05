# frozen_string_literal: true

require "test_helper"

# Minitest integration tests for the MCP key authentication extension (section A).
# Mirrors the core cases from spec/requests/api/mcp_auth_spec.rb so CI gates them.
class Api::McpAuthTest < ActionDispatch::IntegrationTest
  setup do
    @workspace = Workspace.create!(name: "MCP Auth WS", slug: "mcp-auth-#{SecureRandom.hex(4)}")
    @user = @workspace.users.create!(
      name: "MCP Tester",
      email_address: "mcp-#{SecureRandom.hex(4)}@example.com",
      password: "password123"
    )
    @client = Doorkeeper::Application.create!(
      name:        "MCP Test Client",
      redirect_uri: "",
      confidential: true,
      scopes:       "emails:read tags:read",
      workspace:    @workspace,
      created_by:   @user
    )
    # plaintext_secret is available only right after creation.
    @plaintext_secret = @client.plaintext_secret
    @mcp_key           = "#{@client.uid}.#{@plaintext_secret}"
  end

  # ---- helpers ----------------------------------------------------------------

  def mcp_post(rpc_method, rpc_params = {}, auth_header:)
    post "/api/mcp",
         params: { jsonrpc: "2.0", id: 1, method: rpc_method, params: rpc_params }.to_json,
         headers: { "Authorization" => auth_header, "CONTENT_TYPE" => "application/json" }
  end

  def tools_list(auth_header:)
    mcp_post("tools/list", {}, auth_header: auth_header)
  end

  # ---- tests ------------------------------------------------------------------

  test "MCP key Bearer uid.secret authenticates and returns tools/list" do
    tools_list(auth_header: "Bearer #{@mcp_key}")

    assert_response :ok
    names = JSON.parse(response.body).dig("result", "tools").map { |t| t["name"] }
    assert_includes names, "list_emails"
    assert_includes names, "list_tags"
    # emails:send is not in the app's scopes
    refute_includes names, "send_email"
  end

  test "wrong secret returns 401 invalid_client" do
    tools_list(auth_header: "Bearer #{@client.uid}.wrongsecret")

    assert_response :unauthorized
    assert_equal "invalid_client", JSON.parse(response.body).dig("error", "code")
  end

  test "tools/list under key auth is scoped to the application's own scopes" do
    # Create a second app with a narrower scope to verify filtering.
    narrow_app = Doorkeeper::Application.create!(
      name:        "Narrow Client",
      redirect_uri: "",
      confidential: true,
      scopes:       "tags:read",
      workspace:    @workspace,
      created_by:   @user
    )
    narrow_secret = narrow_app.plaintext_secret

    tools_list(auth_header: "Bearer #{narrow_app.uid}.#{narrow_secret}")

    assert_response :ok
    names = JSON.parse(response.body).dig("result", "tools").map { |t| t["name"] }
    assert_includes names, "list_tags"
    refute_includes names, "list_emails"
  end

  test "plain Doorkeeper bearer token still works unchanged" do
    # Mint a standard short-lived token for the same user/workspace.
    token = Doorkeeper::AccessToken.create!(
      application: @client,
      expires_in:  2.hours.to_i,
      scopes:      "emails:read tags:read"
    )
    tools_list(auth_header: "Bearer #{token.plaintext_token}")

    assert_response :ok
    names = JSON.parse(response.body).dig("result", "tools").map { |t| t["name"] }
    assert_includes names, "list_emails"
  end
end
