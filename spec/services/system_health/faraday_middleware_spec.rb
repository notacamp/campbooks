# frozen_string_literal: true

require "rails_helper"

RSpec.describe SystemHealth::FaradayMiddleware do
  def build_conn(service: "test_svc", expected_statuses: [], raise_error: false, &stubs_block)
    stubs = Faraday::Adapter::Test::Stubs.new(&stubs_block)
    Faraday.new(url: "http://example.com") do |f|
      f.use SystemHealth::FaradayMiddleware, service: service, expected_statuses: expected_statuses
      f.response :raise_error if raise_error
      f.adapter :test, stubs
    end
  end

  # ── 200 success ───────────────────────────────────────────────────────────────

  it "200 response creates a success row with duration and operation" do
    conn = build_conn do |s|
      s.get("/v1/messages") { [ 200, {}, "ok" ] }
    end

    expect { conn.get("/v1/messages") }.to change(ExternalServiceCall, :count).by(1)

    row = ExternalServiceCall.last
    expect(row).to be_status_success
    expect(row.http_status).to eq(200)
    expect(row.duration_ms).to be >= 0
    expect(row.operation).to eq("GET /v1/messages")
  end

  # ── 500 without raise_error ───────────────────────────────────────────────────

  it "500 response without raise_error creates an error row with http_status 500" do
    conn = build_conn do |s|
      s.get("/v1/messages") { [ 500, {}, "error" ] }
    end

    conn.get("/v1/messages")

    row = ExternalServiceCall.last
    expect(row).to be_status_error
    expect(row.http_status).to eq(500)
  end

  # ── 500 with raise_error middleware ──────────────────────────────────────────

  it "500 with raise_error creates error row AND re-raises the Faraday error" do
    conn = build_conn(raise_error: true) do |s|
      s.get("/v1/messages") { [ 500, {}, "error" ] }
    end

    expect { conn.get("/v1/messages") }.to raise_error(Faraday::ServerError)

    row = ExternalServiceCall.last
    expect(row).to be_status_error
    expect(row.http_status).to eq(500)
    expect(row.error_class).to eq("Faraday::ServerError")
  end

  # ── expected_statuses: 410 treated as success ─────────────────────────────────

  it "410 in expected_statuses with raise_error creates success row and still raises" do
    conn = build_conn(expected_statuses: [ 410 ], raise_error: true) do |s|
      s.get("/v1/sync") { [ 410, {}, "gone" ] }
    end

    expect { conn.get("/v1/sync") }.to raise_error(Faraday::ClientError)

    row = ExternalServiceCall.last
    expect(row).to be_status_success, "expected success row for expected 410, got #{row.status}"
    expect(row.http_status).to eq(410)
  end

  # ── path sanitization ─────────────────────────────────────────────────────────

  it "sanitizes path IDs in the operation string" do
    conn = build_conn do |s|
      s.get("/v3/calendars/abc123def456abc999/events/12345") { [ 200, {}, "ok" ] }
    end

    conn.get("/v3/calendars/abc123def456abc999/events/12345")

    row = ExternalServiceCall.last
    expect(row.operation).to eq("GET /v3/calendars/:id/events/:id")
  end

  # ── timeout ───────────────────────────────────────────────────────────────────

  it "timeout error creates error row with nil http_status and exception propagates" do
    stubs = Faraday::Adapter::Test::Stubs.new do |s|
      s.get("/v1/slow") { raise Faraday::TimeoutError, "execution expired" }
    end

    conn = Faraday.new(url: "http://example.com") do |f|
      f.use SystemHealth::FaradayMiddleware, service: "test_svc"
      f.adapter :test, stubs
    end

    expect { conn.get("/v1/slow") }.to raise_error(Faraday::TimeoutError)

    row = ExternalServiceCall.last
    expect(row).to be_status_error
    expect(row.http_status).to be_nil
    expect(row.error_class).to eq("Faraday::TimeoutError")
  end
end
