# frozen_string_literal: true

require "test_helper"

# Tests for CalendarScanJob's response to Calendars::ServiceUnavailable
# (a Google identity with no Calendar provisioned).
class CalendarScanJobTest < ActiveSupport::TestCase
  include ActiveJob::TestHelper

  setup do
    @workspace = create(:workspace)
    # Fresh account with no calendars — refresh_calendar_list is called even on
    # incremental scope because account.calendars.empty? is true.
    @account = create(:calendar_account, workspace: @workspace, provider: :google)
  end

  # Override CalendarAccount#calendar_client for the duration of a block so the
  # job receives a controlled client without making any OAuth token call.
  def with_calendar_client(fake_client)
    original = CalendarAccount.instance_method(:calendar_client)
    CalendarAccount.send(:define_method, :calendar_client) { fake_client }
    yield
  ensure
    CalendarAccount.send(:define_method, :calendar_client, original)
  end

  def raising_client
    fake = Object.new
    fake.define_singleton_method(:calendar_list) do
      raise Calendars::ServiceUnavailable, "Google account is not signed up for Google Calendar"
    end
    fake
  end

  # ── ServiceUnavailable during calendar_list → deactivate, no re-raise ─────────

  test "deactivates account with calendar_service_unavailable when calendar_list raises ServiceUnavailable" do
    with_calendar_client(raising_client) do
      assert_nothing_raised { CalendarScanJob.perform_now(@account.id, "incremental") }
    end

    @account.reload
    assert_not @account.active?, "account must be deactivated"
    assert_equal "calendar_service_unavailable", @account.deactivation_reason
    assert @account.deactivated_for_service?
  end

  test "does not re-raise Calendars::ServiceUnavailable out of the job" do
    with_calendar_client(raising_client) do
      # assert_nothing_raised ensures the job finishes without propagating the error.
      assert_nothing_raised { CalendarScanJob.perform_now(@account.id, "incremental") }
    end
  end

  test "does not deactivate account when calendar_list returns normally" do
    success_client = Object.new
    success_client.define_singleton_method(:calendar_list) { [] }

    with_calendar_client(success_client) do
      CalendarScanJob.perform_now(@account.id, "incremental")
    end

    @account.reload
    assert @account.active?, "account must remain active after a clean sync"
    assert_nil @account.deactivation_reason
  end

  test "already-inactive account is skipped by the active scope so deactivation_reason stays put" do
    @account.deactivate_for!(:calendar_service_unavailable)

    call_count = 0
    counting_client = Object.new
    counting_client.define_singleton_method(:calendar_list) { call_count += 1; [] }

    # The job filters by CalendarAccount.active — an inactive account is excluded,
    # so calendar_client is never called.
    with_calendar_client(counting_client) do
      CalendarScanJob.perform_now(@account.id, "incremental")
    end

    assert_equal 0, call_count
    assert_equal "calendar_service_unavailable", @account.reload.deactivation_reason
  end
end
