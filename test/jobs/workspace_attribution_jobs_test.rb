# frozen_string_literal: true

require "test_helper"

# Verifies that each patched job scopes Current.workspace per unit of work so
# ExternalServiceCall rows are attributed to the correct workspace.
class WorkspaceAttributionJobsTest < ActiveSupport::TestCase
  include ActiveJob::TestHelper

  setup do
    WebMock.disable_net_connect!
    Rails.cache.clear
  end

  teardown do
    WebMock.reset!
    WebMock.allow_net_connect!
    Current.workspace = nil
  end

  # ── Sweep job: CalendarScanJob ────────────────────────────────────────────────
  #
  # A full sweep iterates all active CalendarAccounts. Each account must be
  # scoped to its own workspace so no cross-contamination occurs.

  test "CalendarScanJob attributes ExternalServiceCall rows to the correct workspace per account" do
    ws_a = Workspace.create!(name: "Cal Attr WS A")
    ws_b = Workspace.create!(name: "Cal Attr WS B")

    # Create one Google CalendarAccount per workspace.
    acc_a = CalendarAccount.create!(
      workspace: ws_a,
      email_address: "cal-a-#{SecureRandom.hex(4)}@example.com",
      provider_account_id: SecureRandom.hex(8),
      refresh_token: "tok_a_#{SecureRandom.hex(16)}",
      provider: :google,
      active: true
    )
    acc_b = CalendarAccount.create!(
      workspace: ws_b,
      email_address: "cal-b-#{SecureRandom.hex(4)}@example.com",
      provider_account_id: SecureRandom.hex(8),
      refresh_token: "tok_b_#{SecureRandom.hex(16)}",
      provider: :google,
      active: true
    )

    # Stub OAuth token refresh (google_oauth rows).
    stub_request(:post, "https://oauth2.googleapis.com/token")
      .to_return(
        status: 200,
        body: { "access_token" => "test_token", "expires_in" => 3600 }.to_json,
        headers: { "Content-Type" => "application/json" }
      )

    # Stub Google Calendar list (google_calendar rows). Both accounts hit the
    # same endpoint; return an empty list so there are no calendars to iterate.
    stub_request(:get, %r{\Ahttps://www\.googleapis\.com/calendar/v3/users/me/calendarList})
      .to_return(
        status: 200,
        body: { "items" => [] }.to_json,
        headers: { "Content-Type" => "application/json" }
      )

    before = Time.current

    with_env("GOOGLE_CLIENT_ID" => "test_client_id", "GOOGLE_CLIENT_SECRET" => "test_secret") do
      CalendarScanJob.perform_now(nil, "full")
    end

    ws_a_calls = ExternalServiceCall.where(workspace_id: ws_a.id).where(created_at: before..)
    ws_b_calls = ExternalServiceCall.where(workspace_id: ws_b.id).where(created_at: before..)

    assert ws_a_calls.any?, "expected ExternalServiceCall rows attributed to ws_a"
    assert ws_b_calls.any?, "expected ExternalServiceCall rows attributed to ws_b"

    # No cross-contamination: ws_a rows must all be ws_a, ws_b rows must all be ws_b.
    assert ws_a_calls.all? { |c| c.workspace_id == ws_a.id },
      "all ws_a rows must have workspace_id = ws_a.id"
    assert ws_b_calls.all? { |c| c.workspace_id == ws_b.id },
      "all ws_b rows must have workspace_id = ws_b.id"

    assert_nil Current.workspace, "Current.workspace must be nil after CalendarScanJob#perform"
  end

  # ── Single-record job: Calendars::EventWriteJob ───────────────────────────────
  #
  # Writing a single calendar event must set Current.workspace to the event's
  # workspace so the API call rows are attributed correctly.

  test "Calendars::EventWriteJob attributes ExternalServiceCall rows to the event's workspace" do
    ws = Workspace.create!(name: "EventWrite WS")
    cal_account = CalendarAccount.create!(
      workspace: ws,
      email_address: "ew-#{SecureRandom.hex(4)}@example.com",
      provider_account_id: SecureRandom.hex(8),
      refresh_token: "tok_ew_#{SecureRandom.hex(16)}",
      provider: :google,
      active: true
    )
    calendar = cal_account.calendars.create!(
      provider_calendar_id: "primary",
      name: "Primary Calendar"
    )
    event = calendar.calendar_events.create!(
      provider_event_id: "local-#{SecureRandom.hex(8)}",
      title: "Test event",
      start_at: 1.day.from_now,
      end_at: 1.day.from_now + 1.hour,
      status: :confirmed,
      outbound_pending: true
    )

    stub_request(:post, "https://oauth2.googleapis.com/token")
      .to_return(
        status: 200,
        body: { "access_token" => "test_token", "expires_in" => 3600 }.to_json,
        headers: { "Content-Type" => "application/json" }
      )

    stub_request(:post, %r{\Ahttps://www\.googleapis\.com/calendar/v3/calendars/.*/events})
      .to_return(
        status: 200,
        body: {
          "id"     => "remote-event-#{SecureRandom.hex(4)}",
          "etag"   => "\"etag-test\"",
          "status" => "confirmed"
        }.to_json,
        headers: { "Content-Type" => "application/json" }
      )

    before = Time.current

    with_env("GOOGLE_CLIENT_ID" => "test_client_id", "GOOGLE_CLIENT_SECRET" => "test_secret") do
      Calendars::EventWriteJob.perform_now(event.id, "create")
    end

    ws_calls = ExternalServiceCall.where(workspace_id: ws.id).where(created_at: before..)

    assert ws_calls.any?,
      "expected ExternalServiceCall rows attributed to the event's workspace"
    assert ws_calls.all? { |c| c.workspace_id == ws.id },
      "all rows must be attributed to the event's workspace"

    assert_nil Current.workspace, "Current.workspace must be nil after EventWriteJob#perform"
  end
end
