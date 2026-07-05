# frozen_string_literal: true

require "test_helper"

class SystemHealth::FaradayMiddlewareTest < ActiveSupport::TestCase
  def build_conn(service: "test_svc", expected_statuses: [], raise_error: false, &stubs_block)
    stubs = Faraday::Adapter::Test::Stubs.new(&stubs_block)
    Faraday.new(url: "http://example.com") do |f|
      f.use SystemHealth::FaradayMiddleware, service: service, expected_statuses: expected_statuses
      f.response :raise_error if raise_error
      f.adapter :test, stubs
    end
  end

  # ── 200 success ───────────────────────────────────────────────────────────────

  test "200 response creates a success row with duration and operation" do
    conn = build_conn do |s|
      s.get("/v1/messages") { [ 200, {}, "ok" ] }
    end

    assert_difference("ExternalServiceCall.count") do
      conn.get("/v1/messages")
    end

    row = ExternalServiceCall.last
    assert row.status_success?
    assert_equal 200, row.http_status
    assert row.duration_ms >= 0
    assert_equal "GET /v1/messages", row.operation
  end

  # ── 500 without raise_error ───────────────────────────────────────────────────

  test "500 response without raise_error creates an error row with http_status 500" do
    conn = build_conn do |s|
      s.get("/v1/messages") { [ 500, {}, "error" ] }
    end

    conn.get("/v1/messages")

    row = ExternalServiceCall.last
    assert row.status_error?
    assert_equal 500, row.http_status
  end

  # ── 500 with raise_error middleware ──────────────────────────────────────────

  test "500 with raise_error creates error row AND re-raises the Faraday error" do
    conn = build_conn(raise_error: true) do |s|
      s.get("/v1/messages") { [ 500, {}, "error" ] }
    end

    assert_raises(Faraday::ServerError) do
      conn.get("/v1/messages")
    end

    row = ExternalServiceCall.last
    assert row.status_error?
    assert_equal 500, row.http_status
    assert_equal "Faraday::ServerError", row.error_class
  end

  # ── expected_statuses: 410 treated as success ─────────────────────────────────

  test "410 in expected_statuses with raise_error creates success row and still raises" do
    conn = build_conn(expected_statuses: [ 410 ], raise_error: true) do |s|
      s.get("/v1/sync") { [ 410, {}, "gone" ] }
    end

    assert_raises(Faraday::ClientError) do
      conn.get("/v1/sync")
    end

    row = ExternalServiceCall.last
    assert row.status_success?, "expected success row for expected 410, got #{row.status}"
    assert_equal 410, row.http_status
  end

  # ── path sanitization ─────────────────────────────────────────────────────────

  test "sanitizes path IDs in the operation string" do
    conn = build_conn do |s|
      s.get("/v3/calendars/abc123def456abc999/events/12345") { [ 200, {}, "ok" ] }
    end

    conn.get("/v3/calendars/abc123def456abc999/events/12345")

    row = ExternalServiceCall.last
    assert_equal "GET /v3/calendars/:id/events/:id", row.operation
  end

  # ── timeout ───────────────────────────────────────────────────────────────────

  test "timeout error creates error row with nil http_status and exception propagates" do
    stubs = Faraday::Adapter::Test::Stubs.new do |s|
      s.get("/v1/slow") { raise Faraday::TimeoutError, "execution expired" }
    end

    conn = Faraday.new(url: "http://example.com") do |f|
      f.use SystemHealth::FaradayMiddleware, service: "test_svc"
      f.adapter :test, stubs
    end

    assert_raises(Faraday::TimeoutError) do
      conn.get("/v1/slow")
    end

    row = ExternalServiceCall.last
    assert row.status_error?
    assert_nil row.http_status
    assert_equal "Faraday::TimeoutError", row.error_class
  end
end
