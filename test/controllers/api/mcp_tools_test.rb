# frozen_string_literal: true

require "test_helper"

# Minitest integration tests for the new MCP tools (section B).
# This is the CI gate. It covers a compact happy-path pass for each new tool
# family; edge cases live in spec/requests/api/mcp_spec.rb (RSpec).
class Api::McpToolsTest < ActionDispatch::IntegrationTest
  setup do
    @workspace = Workspace.create!(name: "MCP Tools WS", slug: "mcp-tools-#{SecureRandom.hex(4)}")
    @user = @workspace.users.create!(
      name: "Test User",
      email_address: "mcp-tools-#{SecureRandom.hex(4)}@example.com",
      password: "password123"
    )

    # Broad-scope client so we can exercise all tool families.
    @client = Doorkeeper::Application.create!(
      name:         "MCP Broad Client",
      redirect_uri:  "",
      confidential:  true,
      scopes:        broad_scopes,
      workspace:     @workspace,
      created_by:    @user
    )
    @mcp_key = "#{@client.uid}.#{@client.plaintext_secret}"
  end

  # ---- helpers ---------------------------------------------------------------

  def broad_scopes
    "emails:read emails:write emails:send tags:read tags:write " \
    "document_types:read document_types:write documents:read " \
    "email_accounts:read email_accounts:write contacts:read " \
    "calendar:read calendar:write reminders:read reminders:write " \
    "tasks:read tasks:write folders:read folders:write " \
    "scout:read scout:write scheduled_emails:read scheduled_emails:write"
  end

  def mcp_call(method, params = {})
    post "/api/mcp",
         params: { jsonrpc: "2.0", id: 1, method: method, params: params }.to_json,
         headers: { "Authorization" => "Bearer #{@mcp_key}", "CONTENT_TYPE" => "application/json" }
    assert_response :ok, "Expected 200 for #{method}: #{response.body}"
    JSON.parse(response.body)
  end

  def call_tool(name, arguments = {})
    body = mcp_call("tools/call", { name: name, arguments: arguments })
    content_text = body.dig("result", "content", 0, "text")
    assert_not_nil content_text, "Expected content text for tool #{name}: #{body.inspect}"
    JSON.parse(content_text)
  end

  # ---- tools/list coverage ---------------------------------------------------

  test "tools/list with broad scopes includes the new meta and search tools" do
    body = mcp_call("tools/list", {})
    names = body.dig("result", "tools").map { |t| t["name"] }

    # meta (scope: nil — visible to any authenticated client)
    assert_includes names, "get_overview"
    assert_includes names, "get_setup_status"
    assert_includes names, "guide"

    # new email tools
    assert_includes names, "search_emails"
    assert_includes names, "update_emails"
    assert_includes names, "move_emails_to_folder"
    assert_includes names, "tag_emails"
    assert_includes names, "forward_email"

    # skim
    assert_includes names, "get_skim_deck"
    assert_includes names, "skim_decide"

    # accounts
    assert_includes names, "list_email_accounts"
    assert_includes names, "connect_email_account"

    # taxonomy create
    assert_includes names, "create_tag"
    assert_includes names, "create_document_type"
    assert_includes names, "create_folder"
  end

  test "meta tools are visible even with a narrow scope-only token" do
    narrow_app = Doorkeeper::Application.create!(
      name:         "Narrow",
      redirect_uri:  "",
      confidential:  true,
      scopes:        "tags:read",
      workspace:     @workspace,
      created_by:    @user
    )
    post "/api/mcp",
         params: { jsonrpc: "2.0", id: 1, method: "tools/list", params: {} }.to_json,
         headers: {
           "Authorization" => "Bearer #{narrow_app.uid}.#{narrow_app.plaintext_secret}",
           "CONTENT_TYPE" => "application/json"
         }
    assert_response :ok
    names = JSON.parse(response.body).dig("result", "tools").map { |t| t["name"] }
    assert_includes names, "get_overview",    "meta tool must show with narrow scope"
    assert_includes names, "guide",           "meta tool must show with narrow scope"
    refute_includes names, "search_emails",   "emails:read not granted"
    refute_includes names, "list_documents",  "documents:read not granted"
  end

  # ---- meta tools ------------------------------------------------------------

  test "get_overview returns hint and email section when emails:read is granted" do
    payload = call_tool("get_overview")

    assert_equal "Use guide(topic) to learn workflows; get_setup_status if something looks unconfigured.",
                 payload["hint"]
    assert payload.key?("emails"), "Expected emails section when emails:read scope is granted"
    assert payload["emails"].key?("unread_count")
    assert payload["emails"].key?("skim_pending_count")
  end

  test "get_setup_status returns workspace and next_steps" do
    payload = call_tool("get_setup_status")

    assert_equal @workspace.name, payload.dig("workspace", "name")
    assert payload.key?("ai")
    assert payload.key?("taxonomy")
    assert payload.key?("next_steps")
    assert_kind_of Array, payload["next_steps"]
  end

  test "guide with no topic returns the topics list" do
    payload = call_tool("guide")

    assert payload.key?("topics"), "Expected topics key: #{payload.inspect}"
    names = payload["topics"].map { |t| t["name"] }
    assert_includes names, "getting_started"
    assert_includes names, "triage_and_skim"
    assert_includes names, "context_tips"
  end

  test "guide with a valid topic returns markdown content" do
    payload = call_tool("guide", { "topic" => "getting_started" })

    assert_equal "getting_started", payload["topic"]
    assert payload["content"].present?, "Expected non-empty content for getting_started"
    assert_includes payload["content"], "get_overview"
  end

  test "guide with an unknown topic returns isError" do
    body = mcp_call("tools/call", { name: "guide", arguments: { "topic" => "not_a_topic" } })
    assert body.dig("result", "isError"), "Expected isError for unknown topic"
  end

  # ---- account tools --------------------------------------------------------

  test "list_email_accounts returns accounts with can_send flag" do
    email_account = @workspace.email_accounts.create!(
      email_address: "test-#{SecureRandom.hex(4)}@example.com",
      provider: :zoho,
      refresh_token: "dummy"
    )
    @user.email_account_users.create!(
      email_account: email_account,
      can_read: true, can_send: true, can_manage: true, owner: true
    )

    payload = call_tool("list_email_accounts")

    assert payload.key?("email_accounts"), "Expected email_accounts key"
    assert payload["count"] >= 1
    account_entry = payload["email_accounts"].find { |a| a["id"] == email_account.id }
    assert_not_nil account_entry, "Expected to find the created account"
    assert account_entry["can_send"]
  end

  test "connect_email_account web mode returns the connect_path" do
    payload = call_tool("connect_email_account", { "mode" => "web" })

    assert_equal "/email_accounts/new", payload["connect_path"]
    assert payload["note"].present?
  end

  # ---- taxonomy create tools ------------------------------------------------

  test "create_tag creates a tag and returns it" do
    tag_name = "MCP-tag-#{SecureRandom.hex(4)}"
    payload = call_tool("create_tag", { "name" => tag_name })

    assert payload.key?("tag"), "Expected tag key: #{payload.inspect}"
    assert_equal tag_name, payload.dig("tag", "name")
    assert @workspace.tags.exists?(name: tag_name)
  end

  test "create_tag duplicate returns isError" do
    tag = @workspace.tags.create!(name: "Duplicate", color: "#595dec", source: :local)

    body = mcp_call("tools/call", { name: "create_tag", arguments: { "name" => tag.name } })
    assert body.dig("result", "isError"), "Expected isError on duplicate tag"
  end

  test "create_document_type creates and returns the type" do
    dt_name = "MCP-doctype-#{SecureRandom.hex(4)}"
    payload = call_tool("create_document_type", { "name" => dt_name })

    assert payload.key?("document_type"), "Expected document_type key: #{payload.inspect}"
    assert_equal dt_name, payload.dig("document_type", "name")
    assert @workspace.document_types.exists?(name: dt_name)
  end

  test "create_folder creates and returns the folder" do
    folder_name = "MCP-folder-#{SecureRandom.hex(4)}"
    payload = call_tool("create_folder", { "name" => folder_name })

    assert payload.key?("folder"), "Expected folder key: #{payload.inspect}"
    assert_equal folder_name, payload.dig("folder", "name")
    assert @workspace.mail_folders.exists?(name: folder_name)
  end

  # ---- search_emails tool ---------------------------------------------------

  test "search_emails returns matching emails" do
    ea = @workspace.email_accounts.create!(
      email_address: "inbox-#{SecureRandom.hex(4)}@example.com",
      provider: :zoho,
      refresh_token: "dummy"
    )
    @user.email_account_users.create!(email_account: ea, can_read: true, can_send: false, can_manage: false, owner: true)
    # Disable semantic search — we only test the keyword/browse path
    ea.email_messages.create!(
      provider_message_id: SecureRandom.hex, subject: "Unique MCP invoice test subject",
      from_address: "vendor@example.com", to_address: "me@example.com",
      received_at: 1.hour.ago, body: "Please pay invoice #999"
    )

    # search_emails falls back to the browse scope when the query matches an unusual string
    payload = call_tool("search_emails", { "query" => "Unique MCP invoice test subject" })

    assert payload.key?("emails"), "Expected emails key: #{payload.inspect}"
    assert_kind_of Integer, payload["count"]
  end

  # ---- tasks tools (feature-gated) ------------------------------------------

  test "task tools are hidden when the tasks feature is off" do
    # ENABLE_TASKS is unset in the test env, so Features.tasks? is already false.
    body = mcp_call("tools/list", {})
    names = body.dig("result", "tools").map { |t| t["name"] }
    refute_includes names, "list_tasks", "list_tasks should not appear when tasks? is off"
  end

  # ---- Fix 1: id params must be string (UUID PKs) ---------------------------

  test "all id and *_id inputSchema properties are typed string" do
    Mcp::Registry.all.each do |tool|
      props = (tool.input_schema[:properties] || {})
      props.each do |key, schema|
        next unless key.to_s == "id" || key.to_s.end_with?("_id")
        next unless schema.is_a?(Hash)
        assert_equal "string", schema[:type],
                     "#{tool.name}.#{key} should be type:string but got #{schema[:type].inspect}"
      end
    end
  end

  test "id_schema helper produces type string" do
    tool = Mcp::Registry.find("mark_email_read")
    assert_equal "string", tool.input_schema.dig(:properties, :id, :type)
  end

  # ---- Fix 2: create_task without all_day does not raise NOT NULL -----------

  test "create_task without all_day succeeds" do
    saved = ENV["ENABLE_TASKS"]
    ENV["ENABLE_TASKS"] = "1"
    begin
      @workspace.update!(plan: "pro")
      payload = call_tool("create_task", { "title" => "Test no all_day" })
      assert payload.key?("task"), "Expected task key: #{payload.inspect}"
      assert_equal "Test no all_day", payload.dig("task", "title")
    ensure
      saved.nil? ? ENV.delete("ENABLE_TASKS") : (ENV["ENABLE_TASKS"] = saved)
    end
  end

  # ---- Fix 3: update_task with bogus status returns isError -----------------

  test "update_task with invalid status returns isError" do
    saved = ENV["ENABLE_TASKS"]
    ENV["ENABLE_TASKS"] = "1"
    begin
      @workspace.update!(plan: "pro")
      task = @workspace.tasks.create!(title: "Status guard", status: :todo, created_by: @user)
      body = mcp_call("tools/call", {
        name: "update_task",
        arguments: { "id" => task.id, "status" => "definitely_not_valid" }
      })
      assert body.dig("result", "isError"), "Expected isError for bogus status"
      text = body.dig("result", "content", 0, "text")
      assert_includes text, "Invalid status"
    ensure
      saved.nil? ? ENV.delete("ENABLE_TASKS") : (ENV["ENABLE_TASKS"] = saved)
    end
  end

  # ---- Fix 5: list tools include count field --------------------------------

  test "list_emails returns a count field" do
    ea = @workspace.email_accounts.create!(
      email_address: "count-test-#{SecureRandom.hex(4)}@example.com",
      provider: :zoho, refresh_token: "dummy"
    )
    @user.email_account_users.create!(email_account: ea, can_read: true, can_send: false, can_manage: false, owner: true)
    ea.email_messages.create!(
      provider_message_id: SecureRandom.hex, subject: "Count test email",
      from_address: "x@example.com", to_address: "me@example.com",
      received_at: 1.hour.ago
    )

    payload = call_tool("list_emails")
    assert payload.key?("count"), "Expected count key in list_emails response"
    assert_kind_of Integer, payload["count"]
  end

  test "list_tags returns a count field" do
    @workspace.tags.create!(name: "TagForCount", color: "#595dec", source: :local)
    payload = call_tool("list_tags")
    assert payload.key?("count"), "Expected count key in list_tags response"
    assert_kind_of Integer, payload["count"]
  end

  test "list_document_types returns a count field" do
    payload = call_tool("list_document_types")
    assert payload.key?("count"), "Expected count key in list_document_types response"
    assert_kind_of Integer, payload["count"]
  end

  test "list_folders returns a count field" do
    @workspace.mail_folders.create!(name: "FolderForCount")
    payload = call_tool("list_folders")
    assert payload.key?("count"), "Expected count key in list_folders response"
    assert_kind_of Integer, payload["count"]
  end
end
