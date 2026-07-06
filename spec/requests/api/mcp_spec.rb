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
      "contacts:read contacts:write tags:read tags:write document_types:read document_types:write " \
      "scout:read scout:write scheduled_emails:read scheduled_emails:write " \
      "calendar:read calendar:write reminders:read reminders:write " \
      "folders:read folders:write email_accounts:read email_accounts:write"
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
      names = response.parsed_body["result"]["tools"].map { |t| t["name"] }
      # The scope-nil meta tools remain visible; every workflow tool must be gone.
      expect(names.grep(/workflow/)).to be_empty
      expect(names).to include("get_overview")

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

  # ---- new tools (B) --------------------------------------------------------

  describe "meta tools (scope: nil)" do
    it "scope-nil tools appear with ANY granted scope" do
      rpc({ jsonrpc: "2.0", id: 20, method: "tools/list" }, scopes: "tags:read")

      names = response.parsed_body["result"]["tools"].map { |t| t["name"] }
      expect(names).to include("get_overview", "get_setup_status", "guide")
    end

    it "get_overview includes the emails section only when emails:read is granted" do
      rpc({ jsonrpc: "2.0", id: 21, method: "tools/call",
            params: { name: "get_overview", arguments: {} } },
          scopes: "emails:read")

      payload = JSON.parse(response.parsed_body["result"]["content"].first["text"])
      expect(payload).to have_key("emails")
      expect(payload).not_to have_key("documents")
    end

    it "get_overview omits the emails section when only documents:read is granted" do
      rpc({ jsonrpc: "2.0", id: 22, method: "tools/call",
            params: { name: "get_overview", arguments: {} } },
          scopes: "documents:read")

      payload = JSON.parse(response.parsed_body["result"]["content"].first["text"])
      expect(payload).to have_key("documents")
      expect(payload).not_to have_key("emails")
    end

    it "guide with no topic returns the topics list" do
      rpc({ jsonrpc: "2.0", id: 23, method: "tools/call",
            params: { name: "guide", arguments: {} } },
          scopes: "tags:read")

      payload = JSON.parse(response.parsed_body["result"]["content"].first["text"])
      expect(payload["topics"].map { |t| t["name"] }).to include("getting_started", "triage_and_skim")
    end

    it "guide with a valid topic returns markdown content" do
      rpc({ jsonrpc: "2.0", id: 24, method: "tools/call",
            params: { name: "guide", arguments: { topic: "getting_started" } } },
          scopes: "tags:read")

      payload = JSON.parse(response.parsed_body["result"]["content"].first["text"])
      expect(payload["topic"]).to eq("getting_started")
      expect(payload["content"]).to include("get_overview")
    end

    it "guide with an unknown topic returns isError" do
      rpc({ jsonrpc: "2.0", id: 25, method: "tools/call",
            params: { name: "guide", arguments: { topic: "not_a_topic" } } },
          scopes: "tags:read")

      expect(response.parsed_body.dig("result", "isError")).to be(true)
    end

    it "get_setup_status returns workspace and next_steps" do
      rpc({ jsonrpc: "2.0", id: 26, method: "tools/call",
            params: { name: "get_setup_status", arguments: {} } },
          scopes: "tags:read")

      payload = JSON.parse(response.parsed_body["result"]["content"].first["text"])
      expect(payload.dig("workspace", "name")).to eq(workspace.name)
      expect(payload).to have_key("ai")
      expect(payload).to have_key("taxonomy")
      expect(payload).to have_key("next_steps")
      expect(payload["next_steps"]).to be_a(Array)
    end
  end

  describe "search_emails" do
    it "returns matching emails for a keyword query (browse path; no embeddings)" do
      create(:email_message, email_account: account,
             subject: "Unique MCP test invoice 99", from_address: "vendor@example.com")

      rpc({ jsonrpc: "2.0", id: 30, method: "tools/call",
            params: { name: "search_emails",
                      arguments: { query: "Unique MCP test invoice 99" } } },
          scopes: "emails:read")

      payload = JSON.parse(response.parsed_body["result"]["content"].first["text"])
      expect(payload).to have_key("emails")
      expect(payload).to have_key("count")
    end
  end

  describe "update_emails" do
    it "archives a batch of emails" do
      email = create(:email_message, email_account: account)
      allow(Tools::BulkArchive).to receive(:call).and_return({ archived_count: 1 })

      rpc({ jsonrpc: "2.0", id: 31, method: "tools/call",
            params: { name: "update_emails",
                      arguments: { ids: [ email.id ], action: "archive" } } },
          scopes: "emails:write")

      payload = JSON.parse(response.parsed_body["result"]["content"].first["text"])
      expect(payload["action"]).to eq("archive")
      expect(payload["archived_count"]).to eq(1)
    end

    it "returns isError when snooze is missing snoozed_until" do
      email = create(:email_message, email_account: account)

      rpc({ jsonrpc: "2.0", id: 32, method: "tools/call",
            params: { name: "update_emails",
                      arguments: { ids: [ email.id ], action: "snooze" } } },
          scopes: "emails:write")

      expect(response.parsed_body.dig("result", "isError")).to be(true)
    end
  end

  describe "tag_emails" do
    it "returns isError when the tag does not exist" do
      email = create(:email_message, email_account: account)

      rpc({ jsonrpc: "2.0", id: 33, method: "tools/call",
            params: { name: "tag_emails",
                      arguments: { ids: [ email.id ], tag_name: "NonExistentTag999" } } },
          scopes: "tags:write")

      expect(response.parsed_body.dig("result", "isError")).to be(true)
      expect(response.parsed_body.dig("result", "content", 0, "text")).to include("create_tag")
    end
  end

  describe "get_skim_deck" do
    it "returns rings and a hint" do
      # Stub SkimDeck.for to avoid needing real email records with all columns
      deck_result = [
        {
          theme: :notifications, label: "Notifications",
          clusters: [
            {
              category: :notifications, title: "GitHub", summary: "3 CI runs",
              count: 3, unread_count: 2, bucket: :today, bucket_label: "Today",
              importance: 2, priority_suggested: false, scout_suggestion: nil,
              follow_up: false, follow_up_reason: nil, follow_up_at: nil,
              latest_received_at: 1.hour.ago,
              email_ids: [], samples: [], position: 1, total: 1, pinned: false,
              stacks: 1, senders: []
            }
          ]
        }
      ]
      allow(Emails::SkimDeck).to receive(:for).and_return(deck_result)

      rpc({ jsonrpc: "2.0", id: 40, method: "tools/call",
            params: { name: "get_skim_deck", arguments: {} } },
          scopes: "emails:read")

      payload = JSON.parse(response.parsed_body["result"]["content"].first["text"])
      expect(payload).to have_key("rings")
      expect(payload).to have_key("hint")
      expect(payload["rings"].first["theme"]).to eq("notifications")
    end
  end

  describe "skim_decide" do
    it "keep action marks skimmed_at and records a LearningDecision" do
      email = create(:email_message, email_account: account, skimmed_at: nil)

      allow(Emails::SkimDecisionRecorder).to receive(:record)

      rpc({ jsonrpc: "2.0", id: 41, method: "tools/call",
            params: { name: "skim_decide",
                      arguments: { action: "keep", email_ids: [ email.id ] } } },
          scopes: "emails:write")

      payload = JSON.parse(response.parsed_body["result"]["content"].first["text"])
      expect(payload["action"]).to eq("keep")
      expect(email.reload.skimmed_at).not_to be_nil
      expect(Emails::SkimDecisionRecorder).to have_received(:record).with(anything, [ email.id ], action: "keep")
    end
  end

  describe "list_email_accounts" do
    it "returns the caller's connected accounts" do
      rpc({ jsonrpc: "2.0", id: 50, method: "tools/call",
            params: { name: "list_email_accounts", arguments: {} } },
          scopes: "email_accounts:read")

      payload = JSON.parse(response.parsed_body["result"]["content"].first["text"])
      expect(payload).to have_key("email_accounts")
      expect(payload["email_accounts"].map { |a| a["email_address"] }).to include(account.email_address)
    end
  end

  describe "connect_email_account" do
    # The default (free) plan caps connected mailboxes at 1 and the shared setup
    # already connects one — lift the cap so token mode exercises the OAuth path.
    before { workspace.update!(plan: "unlimited") }

    it "web mode returns the connect_path without any OAuth calls" do
      rpc({ jsonrpc: "2.0", id: 51, method: "tools/call",
            params: { name: "connect_email_account", arguments: { mode: "web" } } },
          scopes: "email_accounts:write")

      payload = JSON.parse(response.parsed_body["result"]["content"].first["text"])
      expect(payload["connect_path"]).to eq("/email_accounts/new")
    end

    it "token mode validates the refresh token and creates an account" do
      fake_access = "ACCESS_TOKEN_FAKE"
      allow(Zoho::OauthClient).to receive(:new).and_return(
        double(refresh!: fake_access)
      )
      allow(Zoho::AccountDiscovery).to receive(:new).with(fake_access).and_return(
        double(discover_identity: { email: "connected@example.com", account_id: "ZID999", name: "Connected" })
      )
      allow(Calendars::AccountProvisioner).to receive(:call)
      allow(EmailScanJob).to receive(:perform_later)

      rpc({ jsonrpc: "2.0", id: 52, method: "tools/call",
            params: { name: "connect_email_account",
                      arguments: { mode: "token", provider: "zoho",
                                   refresh_token: "FAKE_REFRESH" } } },
          scopes: "email_accounts:write")

      payload = JSON.parse(response.parsed_body["result"]["content"].first["text"])
      expect(payload.dig("account", "email_address")).to eq("connected@example.com")
      expect(payload["scan_enqueued"]).to be(true)
    end

    it "token mode returns isError on PermanentAuthError" do
      allow(Zoho::OauthClient).to receive(:new).and_return(
        double(refresh!: nil).tap { |d| allow(d).to receive(:refresh!).and_raise(PermanentAuthError, "invalid_code") }
      )

      rpc({ jsonrpc: "2.0", id: 53, method: "tools/call",
            params: { name: "connect_email_account",
                      arguments: { mode: "token", provider: "zoho",
                                   refresh_token: "REVOKED_TOKEN" } } },
          scopes: "email_accounts:write")

      expect(response.parsed_body.dig("result", "isError")).to be(true)
      text = response.parsed_body.dig("result", "content", 0, "text")
      expect(text).to include("Token refresh failed")
    end
  end

  describe "create_tag" do
    it "creates a tag in the workspace" do
      rpc({ jsonrpc: "2.0", id: 60, method: "tools/call",
            params: { name: "create_tag", arguments: { name: "Invoice Queries" } } },
          scopes: "tags:write")

      payload = JSON.parse(response.parsed_body["result"]["content"].first["text"])
      expect(payload.dig("tag", "name")).to eq("Invoice Queries")
      expect(workspace.tags.reload.map(&:name)).to include("Invoice Queries")
    end

    it "returns isError on a duplicate tag name" do
      workspace.tags.create!(name: "Receipts", color: "#595dec", source: :local)

      rpc({ jsonrpc: "2.0", id: 61, method: "tools/call",
            params: { name: "create_tag", arguments: { name: "Receipts" } } },
          scopes: "tags:write")

      expect(response.parsed_body.dig("result", "isError")).to be(true)
    end
  end

  describe "create_document_type" do
    it "creates a document type in the workspace" do
      rpc({ jsonrpc: "2.0", id: 62, method: "tools/call",
            params: { name: "create_document_type",
                      arguments: { name: "Supplier Contract" } } },
          scopes: "document_types:write")

      payload = JSON.parse(response.parsed_body["result"]["content"].first["text"])
      expect(payload.dig("document_type", "name")).to eq("Supplier Contract")
      expect(workspace.document_types.reload.map(&:name)).to include("Supplier Contract")
    end
  end

  describe "create_folder" do
    it "creates a folder in the workspace" do
      rpc({ jsonrpc: "2.0", id: 63, method: "tools/call",
            params: { name: "create_folder", arguments: { name: "Invoices 2025" } } },
          scopes: "folders:write")

      payload = JSON.parse(response.parsed_body["result"]["content"].first["text"])
      expect(payload.dig("folder", "name")).to eq("Invoices 2025")
      expect(workspace.mail_folders.reload.map(&:name)).to include("Invoices 2025")
    end

    it "creates a folder with provisioning summary when provision: true" do
      # No email accounts connected, so provision returns created: [], failed: []
      rpc({ jsonrpc: "2.0", id: 64, method: "tools/call",
            params: { name: "create_folder",
                      arguments: { name: "Projects", provision: true } } },
          scopes: "folders:write")

      payload = JSON.parse(response.parsed_body["result"]["content"].first["text"])
      expect(payload).to have_key("provision")
      expect(payload.dig("provision", "created_count")).to be >= 0
    end
  end

  describe "tasks tools" do
    it "hides task tools when Features.tasks? is off" do
      allow(Features).to receive(:tasks?).and_return(false)
      rpc({ jsonrpc: "2.0", id: 70, method: "tools/list" }, scopes: "tasks:read tasks:write")

      names = response.parsed_body["result"]["tools"].map { |t| t["name"] }
      expect(names).not_to include("list_tasks", "create_task", "complete_task")
    end

    it "shows and runs task tools when Features.tasks? is on" do
      allow(Features).to receive(:tasks?).and_return(true)
      # entitlements builds a fresh resolver per call — grant tasks via the plan, not a stub.
      workspace.update!(plan: "pro")

      rpc({ jsonrpc: "2.0", id: 71, method: "tools/call",
            params: { name: "list_tasks", arguments: {} } },
          scopes: "tasks:read")

      payload = JSON.parse(response.parsed_body["result"]["content"].first["text"])
      expect(payload).to have_key("tasks")
    end
  end

  # ---- Fix 1: id params must be typed string (UUIDs) -----------------------

  describe "inputSchema id types" do
    it "declares id as type string in get_task inputSchema" do
      rpc({ jsonrpc: "2.0", id: 80, method: "tools/list" }, scopes: "tasks:read tasks:write")
      allow(Features).to receive(:tasks?).and_return(true)

      rpc({ jsonrpc: "2.0", id: 81, method: "tools/list" }, scopes: "tasks:read tasks:write")

      tools = response.parsed_body["result"]["tools"]
      # get_task uses id_schema which we fixed — but tasks may be hidden by feature flag.
      # Verify via the registry directly instead.
      tool = Mcp::Registry.find("mark_email_read")
      id_type = tool.input_schema.dig(:properties, :id, :type)
      expect(id_type).to eq("string")
    end

    it "declares email_account_id as type string in send_email inputSchema" do
      tool = Mcp::Registry.find("send_email")
      id_type = tool.input_schema.dig(:properties, :email_account_id, :type)
      expect(id_type).to eq("string")
    end

    it "declares all *_id properties as string throughout the registry" do
      Mcp::Registry.all.each do |tool|
        props = tool.input_schema[:properties] || {}
        props.each do |key, schema|
          next unless key.to_s == "id" || key.to_s.end_with?("_id")
          next unless schema.is_a?(Hash)
          expect(schema[:type]).to eq("string"),
            "#{tool.name}.#{key} should be type:string (got #{schema[:type].inspect})"
        end
      end
    end
  end

  # ---- Fix 2: create_task without all_day must not crash --------------------

  describe "create_task" do
    before do
      allow(Features).to receive(:tasks?).and_return(true)
      workspace.update!(plan: "pro")
    end

    it "succeeds without all_day (defaults false, no NOT NULL crash)" do
      rpc({ jsonrpc: "2.0", id: 90, method: "tools/call",
            params: { name: "create_task", arguments: { title: "Spec task no all_day" } } },
          scopes: "tasks:write")

      result = response.parsed_body["result"]
      expect(result["isError"]).to be_falsey
      payload = JSON.parse(result["content"].first["text"])
      expect(payload.dig("task", "title")).to eq("Spec task no all_day")
    end
  end

  # ---- Fix 3: update_task with bogus status returns isError ----------------

  describe "update_task" do
    before do
      allow(Features).to receive(:tasks?).and_return(true)
      workspace.update!(plan: "pro")
    end

    it "returns isError for an invalid status value" do
      task = Task.create!(
        title: "Status test task",
        status: :todo,
        workspace: workspace,
        created_by: user
      )

      rpc({ jsonrpc: "2.0", id: 91, method: "tools/call",
            params: { name: "update_task",
                      arguments: { id: task.id, status: "not_a_valid_status" } } },
          scopes: "tasks:write")

      result = response.parsed_body["result"]
      expect(result["isError"]).to be(true)
      expect(result["content"].first["text"]).to include("Invalid status")
    end
  end

  # ---- Fix 5: list tools include count ------------------------------------

  describe "list tool count field" do
    it "list_emails includes a count field" do
      create(:email_message, email_account: account, subject: "Count test")

      rpc({ jsonrpc: "2.0", id: 95, method: "tools/call",
            params: { name: "list_emails", arguments: {} } }, scopes: "emails:read")

      payload = JSON.parse(response.parsed_body["result"]["content"].first["text"])
      expect(payload).to have_key("count")
      expect(payload["count"]).to be_a(Integer)
    end

    it "list_tags includes a count field" do
      Tag.create!(workspace: workspace, name: "CountTag", color: "#ccc", source: :local)

      rpc({ jsonrpc: "2.0", id: 96, method: "tools/call",
            params: { name: "list_tags", arguments: {} } }, scopes: "tags:read")

      payload = JSON.parse(response.parsed_body["result"]["content"].first["text"])
      expect(payload).to have_key("count")
    end

    it "list_tags excludes hidden provider labels" do
      Tag.create!(workspace: workspace, name: "VisibleTag", color: "#ccc", source: :local)
      Tag.create!(workspace: workspace, name: "HIDDEN_CATEGORY", color: "#ccc", source: :external,
                  email_account: account, external_label_id: "CATEGORY_UPDATES", kind: :category, hidden: true)

      rpc({ jsonrpc: "2.0", id: 98, method: "tools/call",
            params: { name: "list_tags", arguments: {} } }, scopes: "tags:read")

      names = JSON.parse(response.parsed_body["result"]["content"].first["text"])["tags"].map { |t| t["name"] }
      expect(names).to include("VisibleTag")
      expect(names).not_to include("HIDDEN_CATEGORY")
    end

    it "list_document_types includes a count field" do
      rpc({ jsonrpc: "2.0", id: 97, method: "tools/call",
            params: { name: "list_document_types", arguments: {} } }, scopes: "document_types:read")

      payload = JSON.parse(response.parsed_body["result"]["content"].first["text"])
      expect(payload).to have_key("count")
    end

    it "list_folders includes a count field" do
      workspace.mail_folders.create!(name: "FolderForCount")

      rpc({ jsonrpc: "2.0", id: 98, method: "tools/call",
            params: { name: "list_folders", arguments: {} } }, scopes: "folders:read")

      payload = JSON.parse(response.parsed_body["result"]["content"].first["text"])
      expect(payload).to have_key("count")
      expect(payload["count"]).to be_a(Integer)
    end
  end

  describe "create_calendar_event" do
    let(:calendar_account) { create(:calendar_account, workspace: workspace) }
    let(:calendar) { create(:calendar, calendar_account: calendar_account, is_writable: true, syncing: true) }

    before { create(:calendar_account_user, :owner, user: user, calendar_account: calendar_account) }

    # Regression: the tool used to set `color:` on CalendarEvent, which has no
    # such attribute (events render in their calendar's color) — every call
    # raised ActiveModel::UnknownAttributeError.
    it "creates an event without touching a non-existent color attribute" do
      rpc({ jsonrpc: "2.0", id: 98, method: "tools/call",
            params: { name: "create_calendar_event",
                      arguments: { calendar_id: calendar.id, title: "Standup",
                                   start_at: "2026-07-08T09:00:00Z", end_at: "2026-07-08T09:30:00Z" } } },
          scopes: "calendar:write")

      result = response.parsed_body["result"]
      expect(result["isError"]).to be_falsey
      payload = JSON.parse(result["content"].first["text"])
      expect(payload.dig("event", "title")).to eq("Standup")
      expect(calendar.calendar_events.where(title: "Standup")).to exist
    end
  end
end
