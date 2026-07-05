# frozen_string_literal: true

require "test_helper"

class Campbooks::SystemHealth::CallRowTest < ActiveSupport::TestCase
  def render_row(call)
    ApplicationController.render(
      Campbooks::SystemHealth::CallRow.new(call: call),
      layout: false
    )
  end

  test "success row shows http status and duration" do
    call = ExternalServiceCall.new(
      service:     "google_mail",
      status:      :success,
      operation:   "GET /gmail/v1/users/me/messages",
      duration_ms: 320,
      http_status: 200,
      created_at:  1.hour.ago
    )
    html = render_row(call)
    assert_includes html, "200"
    assert_includes html, "320"
  end

  test "error row shows error message line" do
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
    assert_includes html, "rate limit exceeded"
    assert_includes html, "TooManyRequestsError"
  end

  test "nil workspace does not raise" do
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
    assert_nothing_raised { render_row(call) }
  end

  test "nil duration renders dash" do
    call = ExternalServiceCall.new(
      service:     "smtp",
      status:      :success,
      operation:   "send",
      duration_ms: nil,
      created_at:  1.minute.ago
    )
    html = render_row(call)
    assert_includes html, "—"
  end

  # ── href kwarg ────────────────────────────────────────────────────────────────

  test "without href renders a div (non-link)" do
    call = ExternalServiceCall.new(
      service: "google_mail", status: :success, operation: "GET /messages",
      duration_ms: 100, created_at: 1.hour.ago
    )
    html = ApplicationController.render(
      Campbooks::SystemHealth::CallRow.new(call: call),
      layout: false
    )
    # Default (no href) renders as a block div, not an anchor
    assert_includes html, "<div"
    assert_not_includes html, "<a "
  end

  test "with href wraps the row content in a block link" do
    call = ExternalServiceCall.new(
      service: "google_mail", status: :success, operation: "GET /messages",
      duration_ms: 100, created_at: 1.hour.ago
    )
    html = ApplicationController.render(
      Campbooks::SystemHealth::CallRow.new(call: call, href: "/admin/system_health/calls/some-id"),
      layout: false
    )
    assert_includes html, "<a "
    assert_includes html, 'href="/admin/system_health/calls/some-id"'
    assert_includes html, "block hover:bg-muted/50"
  end
end
