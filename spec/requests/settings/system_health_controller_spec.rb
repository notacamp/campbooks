# frozen_string_literal: true

require "rails_helper"

RSpec.describe "Settings::SystemHealthController", type: :request do
  let!(:workspace_a) { Workspace.create!(name: "SH Ctrl WS A") }
  let!(:workspace_b) { Workspace.create!(name: "SH Ctrl WS B") }

  # A plain workspace member (role: member, not admin).
  let!(:member) do
    workspace_a.users.create!(
      name: "SH Member",
      email_address: "sh-member-#{SecureRandom.hex(4)}@example.com",
      password: "password123",
      role: :member
    )
  end

  # A workspace admin.
  let!(:admin) do
    workspace_a.users.create!(
      name: "SH Admin",
      email_address: "sh-admin-#{SecureRandom.hex(4)}@example.com",
      password: "password123",
      role: :admin
    )
  end

  # Seed ExternalServiceCall rows in ws_a, ws_b, and nil workspace.
  let!(:ws_a_call) do
    ExternalServiceCall.create!(
      service:      "google_mail",
      status:       :success,
      operation:    "GET /gmail/ws-a-op",
      workspace_id: workspace_a.id
    )
  end
  let!(:ws_b_call) do
    ExternalServiceCall.create!(
      service:      "zoho_mail",
      status:       :success,
      operation:    "GET /zoho/ws-b-op",
      workspace_id: workspace_b.id
    )
  end
  let!(:nil_call) do
    ExternalServiceCall.create!(
      service:      "smtp",
      status:       :success,
      operation:    "nil-ws-op",
      workspace_id: nil
    )
  end

  # ── Authentication & authorization ────────────────────────────────────────────

  it "unauthenticated request redirects to sign-in" do
    get settings_system_health_path
    expect(response).to redirect_to(new_session_path)
  end

  it "member (non-admin) is redirected to settings root" do
    sign_in_as member
    get settings_system_health_path
    expect(response).to redirect_to(settings_root_path)
  end

  it "workspace admin gets 200" do
    sign_in_as admin
    get settings_system_health_path
    expect(response).to have_http_status(:ok)
    expect(response.body).to include("System health")
  end

  # ── Workspace scoping ─────────────────────────────────────────────────────────

  it "shows workspace A calls and not workspace B or nil-workspace calls" do
    sign_in_as admin
    get settings_system_health_path
    expect(response).to have_http_status(:ok)
    expect(response.body).to include("ws-a-op"),  "admin of ws_a should see ws_a call"
    expect(response.body).not_to include("ws-b-op"),  "admin of ws_a must not see ws_b call"
    expect(response.body).not_to include("nil-ws-op"), "admin of ws_a must not see nil-workspace call"
  end

  # ── Filters ───────────────────────────────────────────────────────────────────

  it "service filter narrows results to matching service" do
    ExternalServiceCall.create!(
      service: "google_mail", status: :success,
      operation: "GET /mail-filter-op", workspace_id: workspace_a.id
    )
    ExternalServiceCall.create!(
      service: "ai_openai", status: :success,
      operation: "POST /ai-filter-op", workspace_id: workspace_a.id
    )

    sign_in_as admin
    get settings_system_health_path(service: "google_mail")
    expect(response).to have_http_status(:ok)
    expect(response.body).to include("mail-filter-op")
    expect(response.body).not_to include("ai-filter-op")
  end

  it "turbo_stream format responds for lazy pagination" do
    sign_in_as admin
    get settings_system_health_path, headers: { "Accept" => "text/vnd.turbo-stream.html" }
    expect(response).to have_http_status(:ok)
  end
end
