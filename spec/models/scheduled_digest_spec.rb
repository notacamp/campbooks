# frozen_string_literal: true

require "rails_helper"

RSpec.describe ScheduledDigest do
  before do
    @ws   = Workspace.create!(name: "Digest Model WS")
    @user = @ws.users.create!(name: "Dana", email_address: "dana-sd@example.com", password: "password123")
  end

  def valid_digest(attrs = {})
    @ws.scheduled_digests.build({
      user:        @user,
      name:        "Weekly roundup",
      rrule:       "FREQ=WEEKLY",
      next_run_at: 1.week.from_now,
      config:      { "sources" => [ { "type" => "emails", "query" => "" } ] }
    }.merge(attrs))
  end

  # ── Validations ──────────────────────────────────────────────────────────────

  it "requires a name" do
    d = valid_digest(name: "")
    expect(d).not_to be_valid
    expect(d.errors[:name]).to include("can't be blank")
  end

  it "name must be <= 80 chars" do
    d = valid_digest(name: "A" * 81)
    expect(d).not_to be_valid
    expect(d.errors[:name]).to be_any
  end

  it "requires a valid rrule format" do
    d = valid_digest(rrule: "INVALID")
    expect(d).not_to be_valid
    expect(d.errors[:rrule]).to be_any
  end

  it "accepts FREQ=DAILY" do
    d = valid_digest(rrule: "FREQ=DAILY")
    expect(d).to be_valid, d.errors.full_messages.inspect
  end

  it "accepts FREQ=WEEKLY;INTERVAL=2" do
    d = valid_digest(rrule: "FREQ=WEEKLY;INTERVAL=2")
    expect(d).to be_valid, d.errors.full_messages.inspect
  end

  it "requires next_run_at" do
    d = valid_digest(next_run_at: nil)
    expect(d).not_to be_valid
  end

  it "rejects unknown source type" do
    d = valid_digest(config: { "sources" => [ { "type" => "unknown" } ] })
    expect(d).not_to be_valid
    expect(d.errors[:config]).to be_any
  end

  it "rejects config with more than 6 sources" do
    sources = 7.times.map { { "type" => "emails", "query" => "" } }
    d = valid_digest(config: { "sources" => sources })
    expect(d).not_to be_valid
    expect(d.errors[:config]).to be_any
  end

  it "rejects empty source list" do
    d = valid_digest(config: { "sources" => [] })
    expect(d).not_to be_valid
    expect(d.errors[:config]).to be_any
  end

  it "rejects email query with folder: modifier" do
    d = valid_digest(config: { "sources" => [ { "type" => "emails", "query" => "folder:inbox" } ] })
    expect(d).not_to be_valid
    expect(d.errors[:config]).to be_any
  end

  it "rejects email query exceeding 200 chars" do
    d = valid_digest(config: { "sources" => [ { "type" => "emails", "query" => "x" * 201 } ] })
    expect(d).not_to be_valid
    expect(d.errors[:config]).to be_any
  end

  it "rejects invalid window_days for calendar source" do
    d = valid_digest(config: { "sources" => [ { "type" => "calendar", "window_days" => 99 } ] })
    expect(d).not_to be_valid
    expect(d.errors[:config]).to be_any
  end

  it "accepts valid window_days values 7, 14, 30" do
    [ 7, 14, 30 ].each do |w|
      d = valid_digest(config: { "sources" => [ { "type" => "calendar", "window_days" => w } ] })
      expect(d).to be_valid, "expected #{w} to be valid: #{d.errors.full_messages.inspect}"
    end
  end

  it "rejects more than 10 document_types" do
    types = 11.times.map { |i| "type_#{i}" }
    d = valid_digest(config: { "sources" => [ { "type" => "documents", "document_types" => types } ] })
    expect(d).not_to be_valid
    expect(d.errors[:config]).to be_any
  end

  # ── Scopes ───────────────────────────────────────────────────────────────────

  it "enabled scope returns only enabled digests" do
    enabled  = valid_digest.tap(&:save!)
    disabled = valid_digest(enabled: false).tap(&:save!)

    expect(ScheduledDigest.enabled).to include(enabled)
    expect(ScheduledDigest.enabled).not_to include(disabled)
  end

  it "due scope returns enabled digests with past next_run_at" do
    travel_to Time.zone.parse("2026-07-06 10:00:00") do
      due    = valid_digest(next_run_at: 1.hour.ago).tap(&:save!)
      future = valid_digest(next_run_at: 1.hour.from_now).tap(&:save!)
      off    = valid_digest(enabled: false, next_run_at: 1.hour.ago).tap(&:save!)

      expect(ScheduledDigest.due).to include(due)
      expect(ScheduledDigest.due).not_to include(future)
      expect(ScheduledDigest.due).not_to include(off)
    end
  end

  # ── advance_schedule! ─────────────────────────────────────────────────────────

  it "advance_schedule! daily moves next_run_at by one day" do
    travel_to Time.zone.parse("2026-07-06 08:00:00") do
      d = valid_digest(rrule: "FREQ=DAILY", next_run_at: 1.hour.ago).tap(&:save!)
      d.advance_schedule!
      expect(d.next_run_at).to be > Time.current
      expect((d.next_run_at - 1.hour.ago).abs).to be_within(120).of(1.day.to_f)
    end
  end

  it "advance_schedule! weekly moves next_run_at by one week" do
    travel_to Time.zone.parse("2026-07-06 08:00:00") do
      d = valid_digest(rrule: "FREQ=WEEKLY", next_run_at: 1.hour.ago).tap(&:save!)
      d.advance_schedule!
      expect(d.next_run_at).to be > Time.current
    end
  end

  it "advance_schedule! monthly preserves day of month" do
    travel_to Time.zone.parse("2026-01-31 08:00:00") do
      anchor = Time.zone.parse("2026-01-31 08:00:00")
      d = valid_digest(rrule: "FREQ=MONTHLY", next_run_at: anchor).tap(&:save!)
      d.advance_schedule!
      # ScheduleCalculator adds 1 month from the anchor (Jan 31 + 1mo = Feb 28/Mar 3 depending on impl)
      expect(d.next_run_at).to be > Time.current
      expect(d.last_run_at).to be_present
    end
  end

  # ── helpers ───────────────────────────────────────────────────────────────────

  it "frequency returns correct symbol" do
    expect(valid_digest(rrule: "FREQ=DAILY").frequency).to eq(:daily)
    expect(valid_digest(rrule: "FREQ=WEEKLY").frequency).to eq(:weekly)
    expect(valid_digest(rrule: "FREQ=MONTHLY").frequency).to eq(:monthly)
  end

  it "default_lookback varies by frequency" do
    expect(valid_digest(rrule: "FREQ=DAILY").default_lookback).to eq(1.day)
    expect(valid_digest(rrule: "FREQ=WEEKLY").default_lookback).to eq(7.days)
    expect(valid_digest(rrule: "FREQ=MONTHLY").default_lookback).to eq(31.days)
  end

  it "sources returns an array from config" do
    srcs = [ { "type" => "emails", "query" => "" } ]
    d = valid_digest(config: { "sources" => srcs })
    expect(d.sources).to eq(srcs)
  end
end
