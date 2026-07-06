# frozen_string_literal: true

require "rails_helper"

RSpec.describe SystemHealth::Snapshot do
  # Convenience: create a row at a specific time with explicit created_at.
  def make_call(service:, status:, created_at: Time.current, **attrs)
    ExternalServiceCall.create!(
      service:    service,
      status:     status,
      created_at: created_at,
      **attrs
    )
  end

  def snapshot(window: 24.hours, workspace: nil)
    described_class.new(window: window, workspace: workspace)
  end

  def stat_for(snap, service)
    snap.services.find { |s| s.service == service }
  end

  # ── State thresholds ──────────────────────────────────────────────────────────

  it "idle when no calls in window" do
    snap = snapshot
    # All registry services are present but with 0 calls.
    entry = stat_for(snap, "google_mail")
    expect(entry).not_to be_nil
    expect(entry.state).to eq(:idle)
    expect(entry.total).to eq(0)
  end

  it "healthy when errors are few and rate is low" do
    make_call(service: "google_mail", status: :success, created_at: 1.hour.ago)
    make_call(service: "google_mail", status: :success, created_at: 2.hours.ago)
    make_call(service: "google_mail", status: :error,   created_at: 3.hours.ago)

    entry = stat_for(snapshot, "google_mail")
    expect(entry.state).to eq(:healthy)
  end

  it "degraded when error_rate >= 5% and errors >= 3" do
    # 3 errors out of 30 total = 10% error rate
    3.times  { make_call(service: "zoho_mail", status: :error,   created_at: 1.hour.ago) }
    27.times { make_call(service: "zoho_mail", status: :success, created_at: 1.hour.ago) }

    entry = stat_for(snapshot, "zoho_mail")
    expect(entry.state).to eq(:degraded)
  end

  it "failing when errors >= 3 and zero successes" do
    3.times { make_call(service: "smtp", status: :error, created_at: 30.minutes.ago) }

    entry = stat_for(snapshot, "smtp")
    expect(entry.state).to eq(:failing)
  end

  it "failing when error_rate >= 50% and errors >= 5" do
    5.times { make_call(service: "slack", status: :error,   created_at: 1.hour.ago) }
    5.times { make_call(service: "slack", status: :success, created_at: 1.hour.ago) }

    entry = stat_for(snapshot, "slack")
    expect(entry.state).to eq(:failing)
  end

  # ── Buckets ───────────────────────────────────────────────────────────────────

  it "each service entry always has exactly 24 buckets" do
    make_call(service: "google_mail", status: :success, created_at: 1.hour.ago)

    snap = snapshot
    stat = stat_for(snap, "google_mail")
    expect(stat.buckets.size).to eq(24)
  end

  it "buckets are zero-filled for hours with no data" do
    # Only one call 1 hour ago — the other 23 slots should be zero.
    make_call(service: "google_mail", status: :success, created_at: 1.hour.ago)

    stat = stat_for(snapshot, "google_mail")
    empty_buckets = stat.buckets.select { |b| b.total == 0 }
    expect(empty_buckets.size).to be >= 22
  end

  it "buckets are Bucket structs with starts_at, total, errors" do
    make_call(service: "google_mail", status: :success, created_at: 1.hour.ago)

    stat   = stat_for(snapshot, "google_mail")
    bucket = stat.buckets.first

    expect(bucket).to respond_to(:starts_at)
    expect(bucket).to respond_to(:total)
    expect(bucket).to respond_to(:errors)
  end

  it "buckets are ordered oldest first" do
    make_call(service: "google_mail", status: :success, created_at: 2.hours.ago)

    stat = stat_for(snapshot, "google_mail")
    expect(stat.buckets.first.starts_at).to be < stat.buckets.last.starts_at
  end

  # ── last_error ────────────────────────────────────────────────────────────────

  it "last_error is populated for a service that has errors" do
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
    expect(entry.last_error).not_to be_nil
    expect(entry.last_error.error_class).to eq("Faraday::ServerError")
    expect(entry.last_error.error_message).to eq("server error")
    expect(entry.last_error.http_status).to eq(503)
    expect(entry.last_error.operation).to eq("GET /messages")
  end

  it "last_error is nil for a service with only successes" do
    make_call(service: "notion", status: :success, created_at: 1.hour.ago)

    entry = stat_for(snapshot, "notion")
    expect(entry.last_error).to be_nil
  end

  # ── Registry services always included ────────────────────────────────────────

  it "all registry services appear even with zero rows" do
    snap = snapshot
    known_services = SystemHealth::SERVICES.keys
    present = snap.services.map(&:service)

    known_services.each do |svc|
      expect(present).to include(svc), "expected #{svc} to be present in snapshot services"
    end
  end

  # ── Unknown service ───────────────────────────────────────────────────────────

  it "unknown service seen in window appears with group :other" do
    make_call(service: "custom_provider", status: :success, created_at: 1.hour.ago)

    entry = stat_for(snapshot, "custom_provider")
    expect(entry).not_to be_nil
    expect(entry.group).to eq(:other)
  end

  # ── Sorting ───────────────────────────────────────────────────────────────────

  it "failing services appear before healthy services in sorted list" do
    # Make google_mail failing.
    3.times { make_call(service: "google_mail", status: :error, created_at: 1.hour.ago) }
    # Make zoho_mail healthy.
    2.times { make_call(service: "zoho_mail",  status: :success, created_at: 1.hour.ago) }

    snap    = snapshot
    services = snap.services.map(&:service)

    google_pos = services.index("google_mail")
    zoho_pos   = services.index("zoho_mail")

    expect(google_pos).to be < zoho_pos,
      "expected failing google_mail (#{google_pos}) before healthy zoho_mail (#{zoho_pos})"
  end

  # ── Workspace scoping ─────────────────────────────────────────────────────────

  it "workspace-scoped snapshot counts only its own workspace rows" do
    ws_a = Workspace.create!(name: "Snapshot WS A")
    ws_b = Workspace.create!(name: "Snapshot WS B")

    # ws_a: 3 successes + 1 error on google_mail
    3.times { make_call(service: "google_mail", status: :success, workspace_id: ws_a.id) }
    make_call(service: "google_mail", status: :error, workspace_id: ws_a.id)

    # ws_b: 2 successes on zoho_mail
    2.times { make_call(service: "zoho_mail", status: :success, workspace_id: ws_b.id) }

    # nil workspace: 5 successes on smtp (instance-level, unattributed)
    5.times { make_call(service: "smtp", status: :success, workspace_id: nil) }

    snap_a = snapshot(workspace: ws_a)

    expect(snap_a.totals[:total]).to eq(4),
      "expected only ws_a rows in total (4), got #{snap_a.totals[:total]}"
    expect(snap_a.totals[:errors]).to eq(1),
      "expected 1 error from ws_a, got #{snap_a.totals[:errors]}"

    # Scoped snapshot must NOT include ws_b or nil rows
    services_seen = snap_a.services.map(&:service)
    expect(services_seen).to include("google_mail")
    expect(services_seen).not_to include("zoho_mail"),
      "ws_a snapshot must not include ws_b service zoho_mail"
    expect(services_seen).not_to include("smtp"),
      "ws_a snapshot must not include nil-workspace smtp rows"
  end

  it "workspace-scoped snapshot lists only observed services (no registry zero-fill)" do
    ws = Workspace.create!(name: "Snapshot Observed WS")
    make_call(service: "google_mail", status: :success, workspace_id: ws.id)

    snap = snapshot(workspace: ws)

    expect(snap.services.map(&:service)).to eq([ "google_mail" ]),
      "workspace snapshot must only include services with activity, not the full registry"
  end

  it "instance snapshot (workspace nil) zero-fills all registry services" do
    snap = snapshot(workspace: nil)
    known = SystemHealth::SERVICES.keys

    known.each do |svc|
      expect(snap.services.any? { |s| s.service == svc }).to be(true),
        "instance snapshot must include idle registry service #{svc}"
    end
  end

  # ── totals ────────────────────────────────────────────────────────────────────

  it "totals returns correct aggregate counts" do
    2.times { make_call(service: "google_mail", status: :success, created_at: 1.hour.ago) }
    1.times { make_call(service: "google_mail", status: :error,   created_at: 1.hour.ago) }
    3.times { make_call(service: "smtp",        status: :error,   created_at: 30.minutes.ago) }

    totals = snapshot.totals
    expect(totals[:total]).to eq(6)
    expect(totals[:errors]).to eq(4)
    expect(totals[:services_failing]).to eq(1)   # smtp (3 errors, 0 successes)
  end
end
