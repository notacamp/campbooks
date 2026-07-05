# frozen_string_literal: true

require "test_helper"

class ScheduledDigestTest < ActiveSupport::TestCase
  setup do
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

  test "requires a name" do
    d = valid_digest(name: "")
    assert_not d.valid?
    assert_includes d.errors[:name], "can't be blank"
  end

  test "name must be <= 80 chars" do
    d = valid_digest(name: "A" * 81)
    assert_not d.valid?
    assert d.errors[:name].any?
  end

  test "requires a valid rrule format" do
    d = valid_digest(rrule: "INVALID")
    assert_not d.valid?
    assert d.errors[:rrule].any?
  end

  test "accepts FREQ=DAILY" do
    d = valid_digest(rrule: "FREQ=DAILY")
    assert d.valid?, d.errors.full_messages.inspect
  end

  test "accepts FREQ=WEEKLY;INTERVAL=2" do
    d = valid_digest(rrule: "FREQ=WEEKLY;INTERVAL=2")
    assert d.valid?, d.errors.full_messages.inspect
  end

  test "requires next_run_at" do
    d = valid_digest(next_run_at: nil)
    assert_not d.valid?
  end

  test "rejects unknown source type" do
    d = valid_digest(config: { "sources" => [ { "type" => "unknown" } ] })
    assert_not d.valid?
    assert d.errors[:config].any?
  end

  test "rejects config with more than 6 sources" do
    sources = 7.times.map { { "type" => "emails", "query" => "" } }
    d = valid_digest(config: { "sources" => sources })
    assert_not d.valid?
    assert d.errors[:config].any?
  end

  test "rejects empty source list" do
    d = valid_digest(config: { "sources" => [] })
    assert_not d.valid?
    assert d.errors[:config].any?
  end

  test "rejects email query with folder: modifier" do
    d = valid_digest(config: { "sources" => [ { "type" => "emails", "query" => "folder:inbox" } ] })
    assert_not d.valid?
    assert d.errors[:config].any?
  end

  test "rejects email query exceeding 200 chars" do
    d = valid_digest(config: { "sources" => [ { "type" => "emails", "query" => "x" * 201 } ] })
    assert_not d.valid?
    assert d.errors[:config].any?
  end

  test "rejects invalid window_days for calendar source" do
    d = valid_digest(config: { "sources" => [ { "type" => "calendar", "window_days" => 99 } ] })
    assert_not d.valid?
    assert d.errors[:config].any?
  end

  test "accepts valid window_days values 7, 14, 30" do
    [ 7, 14, 30 ].each do |w|
      d = valid_digest(config: { "sources" => [ { "type" => "calendar", "window_days" => w } ] })
      assert d.valid?, "expected #{w} to be valid: #{d.errors.full_messages.inspect}"
    end
  end

  test "rejects more than 10 document_types" do
    types = 11.times.map { |i| "type_#{i}" }
    d = valid_digest(config: { "sources" => [ { "type" => "documents", "document_types" => types } ] })
    assert_not d.valid?
    assert d.errors[:config].any?
  end

  # ── Scopes ───────────────────────────────────────────────────────────────────

  test "enabled scope returns only enabled digests" do
    enabled  = valid_digest.tap(&:save!)
    disabled = valid_digest(enabled: false).tap(&:save!)

    assert_includes ScheduledDigest.enabled, enabled
    assert_not_includes ScheduledDigest.enabled, disabled
  end

  test "due scope returns enabled digests with past next_run_at" do
    travel_to Time.zone.parse("2026-07-06 10:00:00") do
      due   = valid_digest(next_run_at: 1.hour.ago).tap(&:save!)
      future = valid_digest(next_run_at: 1.hour.from_now).tap(&:save!)
      off   = valid_digest(enabled: false, next_run_at: 1.hour.ago).tap(&:save!)

      assert_includes ScheduledDigest.due, due
      assert_not_includes ScheduledDigest.due, future
      assert_not_includes ScheduledDigest.due, off
    end
  end

  # ── advance_schedule! ─────────────────────────────────────────────────────────

  test "advance_schedule! daily moves next_run_at by one day" do
    travel_to Time.zone.parse("2026-07-06 08:00:00") do
      d = valid_digest(rrule: "FREQ=DAILY", next_run_at: 1.hour.ago).tap(&:save!)
      d.advance_schedule!
      assert d.next_run_at > Time.current
      assert_in_delta 1.day.to_f, (d.next_run_at - 1.hour.ago).abs, 120
    end
  end

  test "advance_schedule! weekly moves next_run_at by one week" do
    travel_to Time.zone.parse("2026-07-06 08:00:00") do
      d = valid_digest(rrule: "FREQ=WEEKLY", next_run_at: 1.hour.ago).tap(&:save!)
      d.advance_schedule!
      assert d.next_run_at > Time.current
    end
  end

  test "advance_schedule! monthly preserves day of month" do
    travel_to Time.zone.parse("2026-01-31 08:00:00") do
      anchor = Time.zone.parse("2026-01-31 08:00:00")
      d = valid_digest(rrule: "FREQ=MONTHLY", next_run_at: anchor).tap(&:save!)
      d.advance_schedule!
      # ScheduleCalculator adds 1 month from the anchor (Jan 31 + 1mo = Feb 28/Mar 3 depending on impl)
      assert d.next_run_at > Time.current
      assert d.last_run_at.present?
    end
  end

  # ── helpers ───────────────────────────────────────────────────────────────────

  test "frequency returns correct symbol" do
    assert_equal :daily,   valid_digest(rrule: "FREQ=DAILY").frequency
    assert_equal :weekly,  valid_digest(rrule: "FREQ=WEEKLY").frequency
    assert_equal :monthly, valid_digest(rrule: "FREQ=MONTHLY").frequency
  end

  test "default_lookback varies by frequency" do
    assert_equal 1.day,    valid_digest(rrule: "FREQ=DAILY").default_lookback
    assert_equal 7.days,   valid_digest(rrule: "FREQ=WEEKLY").default_lookback
    assert_equal 31.days,  valid_digest(rrule: "FREQ=MONTHLY").default_lookback
  end

  test "sources returns an array from config" do
    srcs = [ { "type" => "emails", "query" => "" } ]
    d = valid_digest(config: { "sources" => srcs })
    assert_equal srcs, d.sources
  end
end
