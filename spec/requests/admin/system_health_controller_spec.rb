# frozen_string_literal: true

require "rails_helper"

RSpec.describe "Admin::SystemHealthController", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace, app_admin: false) }
  let(:admin) { create(:user, workspace: workspace, app_admin: true) }

  # ── Authentication & authorization ───────────────────────────────────────

  it "unauthenticated request redirects to sign-in" do
    get admin_system_health_path
    expect(response).to redirect_to(new_session_path)
  end

  it "non-admin user is redirected to root" do
    sign_in_as user
    get admin_system_health_path
    expect(response).to redirect_to(root_path)
  end

  it "app_admin user gets 200" do
    sign_in_as admin
    get admin_system_health_path
    expect(response).to have_http_status(:ok)
    expect(response.body).to include("System Health")
  end

  # ── Filtering ─────────────────────────────────────────────────────────────

  it "service filter narrows results" do
    sign_in_as admin

    create(:external_service_call,
      service: "google_mail", operation: "GET /gmail/messages/mail-only-op")
    create(:external_service_call,
      service: "ai_openai",   operation: "POST /v1/chat/ai-only-op")

    get admin_system_health_path(service: "google_mail")
    expect(response).to have_http_status(:ok)
    expect(response.body).to include("mail-only-op")
    expect(response.body).not_to include("ai-only-op")
  end

  it "status=error filter shows only errors" do
    sign_in_as admin

    create(:external_service_call, service: "google_mail",
      status: :success, operation: "success-op")
    create(:external_service_call, :error, service: "google_mail",
      operation: "error-op")

    get admin_system_health_path(status: "error")
    expect(response).to have_http_status(:ok)
    expect(response.body).to include("error-op")
    expect(response.body).not_to include("success-op")
  end

  it "turbo_stream format responds for lazy pagination" do
    sign_in_as admin
    get admin_system_health_path, headers: { "Accept" => "text/vnd.turbo-stream.html" }
    expect(response).to have_http_status(:ok)
  end

  # ── Call detail page ──────────────────────────────────────────────────────────

  it "app_admin can GET call detail page (200) with a body snippet" do
    call = create(:external_service_call,
      service: "ai_openai",
      request_body: '{"model":"gpt-4","messages":[]}',
      response_body: '{"choices":[]}',
      request_headers: { "Content-Type" => "application/json" },
      response_headers: { "X-Request-Id" => "req-abc" })

    sign_in_as admin
    get call_admin_system_health_path(id: call.id)
    expect(response).to have_http_status(:ok)
    expect(response.body).to include("gpt-4")        # from request_body
    expect(response.body).to include("req-abc")      # from response_headers
  end

  it "workspace-role admin (non-app-admin) cannot access call detail page" do
    call = create(:external_service_call)
    sign_in_as user
    get call_admin_system_health_path(id: call.id)
    expect(response).to redirect_to(root_path)
  end

  it "call detail page renders fine for a row with all-nil capture columns" do
    call = create(:external_service_call,
      request_headers: nil, response_headers: nil,
      request_body: nil, response_body: nil)

    sign_in_as admin
    get call_admin_system_health_path(id: call.id)
    expect(response).to have_http_status(:ok)
    # Should show the "not captured" placeholder for each section
    expect(response.body).to include("(not captured)")
  end
end
