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

  describe "full API surface" do
    let(:all_scopes) do
      "emails:read emails:write emails:send documents:read documents:write " \
      "contacts:read contacts:write tags:read tags:write document_types:read " \
      "scout:read scout:write scheduled_emails:read scheduled_emails:write " \
      "calendar:read calendar:write reminders:read reminders:write folders:read folders:write"
    end

    it "exposes tools across every domain when all scopes are granted" do
      rpc({ jsonrpc: "2.0", id: 10, method: "tools/list" }, scopes: all_scopes)

      names = response.parsed_body["result"]["tools"].map { |t| t["name"] }
      expect(names).to include(
        "mark_email_read", "add_email_tag", "list_documents", "update_document",
        "approve_document", "get_contact", "set_contact_state", "list_tags",
        "list_document_types", "get_scheduled_email", "update_calendar_event",
        "rsvp_calendar_event", "confirm_reminder", "list_folders", "file_document"
      )
      expect(names.size).to be >= 40
    end

    it "hides workflow tools unless the Workflows feature is enabled" do
      allow(Features).to receive(:workflows?).and_return(false)
      rpc({ jsonrpc: "2.0", id: 11, method: "tools/list" }, scopes: "workflows:read workflows:trigger")
      expect(response.parsed_body["result"]["tools"]).to be_empty

      allow(Features).to receive(:workflows?).and_return(true)
      rpc({ jsonrpc: "2.0", id: 12, method: "tools/list" }, scopes: "workflows:read workflows:trigger")
      names = response.parsed_body["result"]["tools"].map { |t| t["name"] }
      expect(names).to include("list_workflows", "trigger_workflow", "list_workflow_executions")
    end

    it "runs a read tool (list_tags)" do
      Tag.create!(workspace: workspace, name: "Receipts", color: "#ccc", source: :local)

      rpc({ jsonrpc: "2.0", id: 13, method: "tools/call",
            params: { name: "list_tags", arguments: {} } }, scopes: "tags:read")

      payload = JSON.parse(response.parsed_body["result"]["content"].first["text"])
      expect(payload["tags"].map { |t| t["name"] }).to include("Receipts")
    end

    it "runs a write tool (mark_email_read)" do
      email = create(:email_message, email_account: account, read: false)

      rpc({ jsonrpc: "2.0", id: 14, method: "tools/call",
            params: { name: "mark_email_read", arguments: { id: email.id } } }, scopes: "emails:write")

      expect(response.parsed_body["result"]["content"]).to be_present
      expect(email.reload.read).to be(true)
    end

    it "runs a state-change tool (set_contact_state)" do
      contact = create(:contact, workspace: workspace)

      rpc({ jsonrpc: "2.0", id: 15, method: "tools/call",
            params: { name: "set_contact_state", arguments: { id: contact.id, state: "star" } } },
          scopes: "contacts:write")

      expect(response.parsed_body.dig("result", "isError")).to be_falsey
      expect(contact.reload.starred?).to be(true)
    end
  end
end
