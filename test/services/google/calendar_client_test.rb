# frozen_string_literal: true

require "test_helper"

# Tests for the provider-not-enabled detection paths in Google::CalendarClient.
# Uses WebMock to simulate the API responses; allocates the client directly so
# no OAuth HTTP call is needed.
class Google::CalendarClientTest < ActiveSupport::TestCase
  CALENDAR_LIST_URL = "https://www.googleapis.com/calendar/v3/users/me/calendarList"

  setup do
    WebMock.disable_net_connect!

    # Allocate without calling initialize — @oauth is injected manually so the
    # token-refresh POST to accounts.google.com is never made.
    @client = Google::CalendarClient.allocate
    fake_oauth = Object.new
    fake_oauth.define_singleton_method(:access_token) { "fake_access_token" }
    @client.instance_variable_set(:@oauth, fake_oauth)
  end

  teardown do
    WebMock.reset!
    WebMock.allow_net_connect!
  end

  # ── 403 "must be signed up for Google Calendar" ──────────────────────────────

  test "calendar_list raises Calendars::ServiceUnavailable on 403 with 'must be signed up' message" do
    stub_request(:get, CALENDAR_LIST_URL)
      .to_return(
        status: 403,
        body: '{"error":{"code":403,"message":"The user must be signed up for Google Calendar."}}',
        headers: { "Content-Type" => "application/json" }
      )

    assert_raises(Calendars::ServiceUnavailable) { @client.calendar_list }
  end

  # ── 403 with a different body must NOT raise ServiceUnavailable ──────────────

  test "calendar_list does not raise Calendars::ServiceUnavailable for a generic 403" do
    stub_request(:get, CALENDAR_LIST_URL)
      .to_return(
        status: 403,
        body: '{"error":{"code":403,"message":"The caller does not have permission"}}',
        headers: { "Content-Type" => "application/json" }
      )

    # A generic 403 raises a plain RuntimeError from raise_for_status!, NOT a
    # ServiceUnavailable.
    err = assert_raises(RuntimeError) { @client.calendar_list }
    assert_not_kind_of Calendars::ServiceUnavailable, err
  end

  # ── 401 raises AuthenticationError, not ServiceUnavailable ───────────────────

  test "calendar_list raises AuthenticationError on 401" do
    stub_request(:get, CALENDAR_LIST_URL)
      .to_return(
        status: 401,
        body: '{"error":{"code":401,"message":"Invalid Credentials"}}',
        headers: { "Content-Type" => "application/json" }
      )

    assert_raises(AuthenticationError) { @client.calendar_list }
  end

  # ── success returns an empty list when items is absent ───────────────────────

  test "calendar_list returns empty array on a successful empty response" do
    stub_request(:get, CALENDAR_LIST_URL)
      .to_return(
        status: 200,
        body: "{}",
        headers: { "Content-Type" => "application/json" }
      )

    assert_equal [], @client.calendar_list
  end
end
