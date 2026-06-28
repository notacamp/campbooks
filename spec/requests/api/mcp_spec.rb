require "rails_helper"

RSpec.describe "API MCP endpoint", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }
  let(:account) { create(:email_account, workspace: workspace) }

  before { create(:email_account_user, user: user, email_account: account, can_read: true, can_send: true) }

  # POST a JSON-RPC message (or a raw body) with a bearer token carrying `scopes`.
  def rpc(payload, scopes: "emails:read", raw: nil)
    headers = api_auth_headers(workspace: workspace, user: user, scopes: scopes)
              .merge("CONTENT_TYPE" => "application/json")
    post api_mcp_path, params: (raw || payload.to_json), headers: headers
  end

  describe "auth" do
    it "requires a bearer token" do
      post api_mcp_path,
           params: { jsonrpc: "2.0", id: 1, method: "initialize" }.to_json,
           headers: { "CONTENT_TYPE" => "application/json" }

      expect(response).to have_http_status(:unauthorized)
    end
  end

  describe "initialize" do
    it "returns the protocol version, tools capability, and server info" do
      rpc({ jsonrpc: "2.0", id: 1, method: "initialize", params: { protocolVersion: "2025-03-26" } })

      expect(response).to have_http_status(:ok)
      result = response.parsed_body["result"]
      expect(result["protocolVersion"]).to eq("2025-03-26")
      expect(result["capabilities"]).to have_key("tools")
      expect(result["serverInfo"]["name"]).to eq("campbooks")
    end
  end

  describe "notifications" do
    it "accepts a notification with no response body" do
      rpc({ jsonrpc: "2.0", method: "notifications/initialized" })

      expect(response).to have_http_status(:accepted)
      expect(response.body).to be_blank
    end
  end

  describe "tools/list" do
    it "lists only the tools the token's scopes allow" do
      rpc({ jsonrpc: "2.0", id: 2, method: "tools/list" }, scopes: "emails:read")

      names = response.parsed_body["result"]["tools"].map { |t| t["name"] }
      expect(names).to include("list_emails", "get_email")
      expect(names).not_to include("send_email", "list_calendar_events", "list_reminders")
    end

    it "exposes write tools when the matching scope is granted" do
      rpc({ jsonrpc: "2.0", id: 2, method: "tools/list" }, scopes: "emails:read emails:send calendar:read")

      names = response.parsed_body["result"]["tools"].map { |t| t["name"] }
      expect(names).to include("send_email", "list_calendar_events")
    end
  end

  describe "tools/call" do
    it "runs a read tool and returns serialized JSON in a text content block" do
      create(:email_message, email_account: account, subject: "Invoice 42")

      rpc({ jsonrpc: "2.0", id: 3, method: "tools/call",
            params: { name: "list_emails", arguments: { limit: 10 } } }, scopes: "emails:read")

      content = response.parsed_body["result"]["content"]
      expect(content.first["type"]).to eq("text")
      payload = JSON.parse(content.first["text"])
      expect(payload["emails"].map { |e| e["subject"] }).to include("Invoice 42")
    end

    it "denies a tool the token lacks scope for (JSON-RPC error)" do
      rpc({ jsonrpc: "2.0", id: 4, method: "tools/call",
            params: { name: "send_email", arguments: {} } }, scopes: "emails:read")

      expect(response.parsed_body["error"]["code"]).to eq(-32_000)
    end

    it "returns invalid params for an unknown tool" do
      rpc({ jsonrpc: "2.0", id: 5, method: "tools/call",
            params: { name: "no_such_tool" } }, scopes: "emails:read")

      expect(response.parsed_body["error"]["code"]).to eq(-32_602)
    end

    it "surfaces a tool failure as isError, not a protocol error" do
      allow(Emails::Sender).to receive(:call).and_return(Emails::Sender::Result.failure("send_failed", "boom"))

      rpc({ jsonrpc: "2.0", id: 6, method: "tools/call",
            params: { name: "send_email",
                      arguments: { email_account_id: account.id, to_address: "x@y.com", body: "hi" } } },
          scopes: "emails:send")

      result = response.parsed_body["result"]
      expect(result["isError"]).to be(true)
      expect(result["content"].first["text"]).to include("boom")
    end
  end

  describe "protocol errors" do
    it "returns a parse error for malformed JSON" do
      rpc(nil, raw: "{ not json")

      expect(response.parsed_body["error"]["code"]).to eq(-32_700)
    end

    it "returns method not found for an unknown method" do
      rpc({ jsonrpc: "2.0", id: 7, method: "bogus/method" })

      expect(response.parsed_body["error"]["code"]).to eq(-32_601)
    end
  end
end
