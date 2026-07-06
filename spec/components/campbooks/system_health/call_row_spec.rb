# frozen_string_literal: true

require "rails_helper"

RSpec.describe Campbooks::SystemHealth::CallRow, type: :component do
  def render_row(call)
    ApplicationController.render(
      described_class.new(call: call),
      layout: false
    )
  end

  it "success row shows http status and duration" do
    call = ExternalServiceCall.new(
      service:     "google_mail",
      status:      :success,
      operation:   "GET /gmail/v1/users/me/messages",
      duration_ms: 320,
      http_status: 200,
      created_at:  1.hour.ago
    )
    html = render_row(call)
    expect(html).to include("200")
    expect(html).to include("320")
  end

  it "error row shows error message line" do
    call = ExternalServiceCall.new(
      service:       "ai_openai",
      status:        :error,
      operation:     "POST /v1/chat/completions",
      duration_ms:   1500,
      http_status:   429,
      error_class:   "Faraday::TooManyRequestsError",
      error_message: "rate limit exceeded",
      created_at:    30.minutes.ago
    )
    html = render_row(call)
    expect(html).to include("rate limit exceeded")
    expect(html).to include("TooManyRequestsError")
  end

  it "nil workspace does not raise" do
    call = ExternalServiceCall.new(
      service:     "google_mail",
      status:      :error,
      operation:   nil,
      duration_ms: nil,
      http_status: nil,
      error_class: "StandardError",
      created_at:  5.minutes.ago,
      workspace:   nil
    )
    expect { render_row(call) }.not_to raise_error
  end

  it "nil duration renders dash" do
    call = ExternalServiceCall.new(
      service:     "smtp",
      status:      :success,
      operation:   "send",
      duration_ms: nil,
      created_at:  1.minute.ago
    )
    html = render_row(call)
    expect(html).to include("—")
  end

  # ── href kwarg ────────────────────────────────────────────────────────────────

  it "without href renders a div (non-link)" do
    call = ExternalServiceCall.new(
      service: "google_mail", status: :success, operation: "GET /messages",
      duration_ms: 100, created_at: 1.hour.ago
    )
    html = ApplicationController.render(
      Campbooks::SystemHealth::CallRow.new(call: call),
      layout: false
    )
    # Default (no href) renders as a block div, not an anchor
    expect(html).to include("<div")
    expect(html).not_to include("<a ")
  end

  it "with href wraps the row content in a block link" do
    call = ExternalServiceCall.new(
      service: "google_mail", status: :success, operation: "GET /messages",
      duration_ms: 100, created_at: 1.hour.ago
    )
    html = ApplicationController.render(
      Campbooks::SystemHealth::CallRow.new(call: call, href: "/admin/system_health/calls/some-id"),
      layout: false
    )
    expect(html).to include("<a ")
    expect(html).to include('href="/admin/system_health/calls/some-id"')
    expect(html).to include("block hover:bg-muted/50")
  end
end
