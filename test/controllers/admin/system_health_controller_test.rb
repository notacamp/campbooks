# frozen_string_literal: true

require "test_helper"

class Admin::SystemHealthControllerTest < ActionDispatch::IntegrationTest
  setup do
    @workspace = create(:workspace)
    @user = create(:user, workspace: @workspace, app_admin: false)
    @admin = create(:user, workspace: @workspace, app_admin: true)
  end

  # ── Authentication & authorization ───────────────────────────────────────

  test "unauthenticated request redirects to sign-in" do
    get admin_system_health_path
    assert_redirected_to new_session_path
  end

  test "non-admin user is redirected to root" do
    sign_in_as @user
    get admin_system_health_path
    assert_redirected_to root_path
  end

  test "app_admin user gets 200" do
    sign_in_as @admin
    get admin_system_health_path
    assert_response :success
    assert_includes response.body, "System Health"
  end

  # ── Filtering ─────────────────────────────────────────────────────────────

  test "service filter narrows results" do
    sign_in_as @admin

    mail_call = create(:external_service_call,
      service: "google_mail", operation: "GET /gmail/messages/mail-only-op")
    ai_call   = create(:external_service_call,
      service: "ai_openai",   operation: "POST /v1/chat/ai-only-op")

    get admin_system_health_path(service: "google_mail")
    assert_response :success
    assert_includes response.body, "mail-only-op"
    assert_not_includes response.body, "ai-only-op"
  end

  test "status=error filter shows only errors" do
    sign_in_as @admin

    create(:external_service_call, service: "google_mail",
      status: :success, operation: "success-op")
    create(:external_service_call, :error, service: "google_mail",
      operation: "error-op")

    get admin_system_health_path(status: "error")
    assert_response :success
    assert_includes response.body, "error-op"
    assert_not_includes response.body, "success-op"
  end

  test "turbo_stream format responds for lazy pagination" do
    sign_in_as @admin
    get admin_system_health_path, headers: { "Accept" => "text/vnd.turbo-stream.html" }
    assert_response :success
  end
end
