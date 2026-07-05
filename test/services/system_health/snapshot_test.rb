# frozen_string_literal: true

require "test_helper"

class SystemHealth::SnapshotTest < ActiveSupport::TestCase
  # Convenience: create a row at a specific time with explicit created_at.
  def make_call(service:, status:, created_at: Time.current, **attrs)
    ExternalServiceCall.create!(
      service:    service,
      status:     status,
      created_at: created_at,
      **attrs
    )
  end

  def snapshot(window: 24.hours)
    SystemHealth::Snapshot.new(window: window)
  end

  def stat_for(snap, service)
    snap.services.find { |s| s.service == service }
  end

  # ── State thresholds ──────────────────────────────────────────────────────────

  test "idle when no calls in window" do
    snap = snapshot
    # All registry services are present but with 0 calls.
    entry = stat_for(snap, "google_mail")
    assert_not_nil entry
    assert_equal :idle, entry.state
    assert_equal 0, entry.total
  end

  test "healthy when errors are few and rate is low" do
    make_call(service: "google_mail", status: :success, created_at: 1.hour.ago)
    make_call(service: "google_mail", status: :success, created_at: 2.hours.ago)
    make_call(service: "google_mail", status: :error,   created_at: 3.hours.ago)

    entry = stat_for(snapshot, "google_mail")
    assert_equal :healthy, entry.state
  end

  test "degraded when error_rate >= 5% and errors >= 3" do
    # 3 errors out of 30 total = 10% error rate
    3.times  { make_call(service: "zoho_mail", status: :error,   created_at: 1.hour.ago) }
    27.times { make_call(service: "zoho_mail", status: :success, created_at: 1.hour.ago) }

    entry = stat_for(snapshot, "zoho_mail")
    assert_equal :degraded, entry.state
  end

  test "failing when errors >= 3 and zero successes" do
    3.times { make_call(service: "smtp", status: :error, created_at: 30.minutes.ago) }

    entry = stat_for(snapshot, "smtp")
    assert_equal :failing, entry.state
  end

  test "failing when error_rate >= 50% and errors >= 5" do
    5.times { make_call(service: "slack", status: :error,   created_at: 1.hour.ago) }
    5.times { make_call(service: "slack", status: :success, created_at: 1.hour.ago) }

    entry = stat_for(snapshot, "slack")
    assert_equal :failing, entry.state
  end

  # ── Buckets ───────────────────────────────────────────────────────────────────

  test "each service entry always has exactly 24 buckets" do
    make_call(service: "google_mail", status: :success, created_at: 1.hour.ago)

    snap = snapshot
    stat = stat_for(snap, "google_mail")
    assert_equal 24, stat.buckets.size
  end

  test "buckets are zero-filled for hours with no data" do
    # Only one call 1 hour ago — the other 23 slots should be zero.
    make_call(service: "google_mail", status: :success, created_at: 1.hour.ago)

    stat = stat_for(snapshot, "google_mail")
    empty_buckets = stat.buckets.select { |b| b.total == 0 }
    assert empty_buckets.size >= 22
  end

  test "buckets are Bucket structs with starts_at, total, errors" do
    make_call(service: "google_mail", status: :success, created_at: 1.hour.ago)

    stat   = stat_for(snapshot, "google_mail")
    bucket = stat.buckets.first

    assert_respond_to bucket, :starts_at
    assert_respond_to bucket, :total
    assert_respond_to bucket, :errors
  end

  test "buckets are ordered oldest first" do
    make_call(service: "google_mail", status: :success, created_at: 2.hours.ago)

    stat = stat_for(snapshot, "google_mail")
    assert stat.buckets.first.starts_at < stat.buckets.last.starts_at
  end

  # ── last_error ────────────────────────────────────────────────────────────────

  test "last_error is populated for a service that has errors" do
    make_call(
      service:       "zoho_mail",
      status:        :error,
      created_at:    30.minutes.ago,
      http_status:   503,
      error_class:   "Faraday::ServerError",
      error_message: "server error",
      operation:     "GET /messages"
    )

    entry = stat_for(snapshot, "zoho_mail")
    assert_not_nil entry.last_error
    assert_equal "Faraday::ServerError", entry.last_error.error_class
    assert_equal "server error",         entry.last_error.error_message
    assert_equal 503,                    entry.last_error.http_status
    assert_equal "GET /messages",        entry.last_error.operation
  end

  test "last_error is nil for a service with only successes" do
    make_call(service: "notion", status: :success, created_at: 1.hour.ago)

    entry = stat_for(snapshot, "notion")
    assert_nil entry.last_error
  end

  # ── Registry services always included ────────────────────────────────────────

  test "all registry services appear even with zero rows" do
    snap = snapshot
    known_services = SystemHealth::SERVICES.keys
    present = snap.services.map(&:service)

    known_services.each do |svc|
      assert_includes present, svc, "expected #{svc} to be present in snapshot services"
    end
  end

  # ── Unknown service ───────────────────────────────────────────────────────────

  test "unknown service seen in window appears with group :other" do
    make_call(service: "custom_provider", status: :success, created_at: 1.hour.ago)

    entry = stat_for(snapshot, "custom_provider")
    assert_not_nil entry
    assert_equal :other, entry.group
  end

  # ── Sorting ───────────────────────────────────────────────────────────────────

  test "failing services appear before healthy services in sorted list" do
    # Make google_mail failing.
    3.times { make_call(service: "google_mail", status: :error, created_at: 1.hour.ago) }
    # Make zoho_mail healthy.
    2.times { make_call(service: "zoho_mail",  status: :success, created_at: 1.hour.ago) }

    snap    = snapshot
    services = snap.services.map(&:service)

    google_pos = services.index("google_mail")
    zoho_pos   = services.index("zoho_mail")

    assert google_pos < zoho_pos,
      "expected failing google_mail (#{google_pos}) before healthy zoho_mail (#{zoho_pos})"
  end

  # ── totals ────────────────────────────────────────────────────────────────────

  test "totals returns correct aggregate counts" do
    2.times { make_call(service: "google_mail", status: :success, created_at: 1.hour.ago) }
    1.times { make_call(service: "google_mail", status: :error,   created_at: 1.hour.ago) }
    3.times { make_call(service: "smtp",        status: :error,   created_at: 30.minutes.ago) }

    totals = snapshot.totals
    assert_equal 6, totals[:total]
    assert_equal 4, totals[:errors]
    assert_equal 1, totals[:services_failing]   # smtp (3 errors, 0 successes)
  end
end
