# frozen_string_literal: true

require "test_helper"

class Settings::SystemHealthControllerTest < ActionDispatch::IntegrationTest
  setup do
    @workspace_a = Workspace.create!(name: "SH Ctrl WS A")
    @workspace_b = Workspace.create!(name: "SH Ctrl WS B")

    # A plain workspace member (role: member, not admin).
    @member = @workspace_a.users.create!(
      name: "SH Member",
      email_address: "sh-member-#{SecureRandom.hex(4)}@example.com",
      password: "password123",
      role: :member
    )

    # A workspace admin.
    @admin = @workspace_a.users.create!(
      name: "SH Admin",
      email_address: "sh-admin-#{SecureRandom.hex(4)}@example.com",
      password: "password123",
      role: :admin
    )

    # Seed ExternalServiceCall rows in ws_a, ws_b, and nil workspace.
    @ws_a_call = ExternalServiceCall.create!(
      service:    "google_mail",
      status:     :success,
      operation:  "GET /gmail/ws-a-op",
      workspace_id: @workspace_a.id
    )
    @ws_b_call = ExternalServiceCall.create!(
      service:    "zoho_mail",
      status:     :success,
      operation:  "GET /zoho/ws-b-op",
      workspace_id: @workspace_b.id
    )
    @nil_call = ExternalServiceCall.create!(
      service:    "smtp",
      status:     :success,
      operation:  "nil-ws-op",
      workspace_id: nil
    )
  end

  # ── Authentication & authorization ────────────────────────────────────────────

  test "unauthenticated request redirects to sign-in" do
    get settings_system_health_path
    assert_redirected_to new_session_path
  end

  test "member (non-admin) is redirected to settings root" do
    sign_in_as @member
    get settings_system_health_path
    assert_redirected_to settings_root_path
  end

  test "workspace admin gets 200" do
    sign_in_as @admin
    get settings_system_health_path
    assert_response :success
    assert_includes response.body, "System health"
  end

  # ── Workspace scoping ─────────────────────────────────────────────────────────

  test "shows workspace A calls and not workspace B or nil-workspace calls" do
    sign_in_as @admin
    get settings_system_health_path
    assert_response :success
    assert_includes response.body,     "ws-a-op",  "admin of ws_a should see ws_a call"
    assert_not_includes response.body, "ws-b-op",  "admin of ws_a must not see ws_b call"
    assert_not_includes response.body, "nil-ws-op", "admin of ws_a must not see nil-workspace call"
  end

  # ── Filters ───────────────────────────────────────────────────────────────────

  test "service filter narrows results to matching service" do
    ExternalServiceCall.create!(
      service: "google_mail", status: :success,
      operation: "GET /mail-filter-op", workspace_id: @workspace_a.id
    )
    ExternalServiceCall.create!(
      service: "ai_openai", status: :success,
      operation: "POST /ai-filter-op", workspace_id: @workspace_a.id
    )

    sign_in_as @admin
    get settings_system_health_path(service: "google_mail")
    assert_response :success
    assert_includes response.body, "mail-filter-op"
    assert_not_includes response.body, "ai-filter-op"
  end

  test "turbo_stream format responds for lazy pagination" do
    sign_in_as @admin
    get settings_system_health_path, headers: { "Accept" => "text/vnd.turbo-stream.html" }
    assert_response :success
  end
end
