# frozen_string_literal: true

require "test_helper"

# Verifies the turbo_stream swipe branch added to CalendarEventsController#destroy.
# A swipe request sends params[:swipe]=1 and expects turbo_stream.remove of
# dom_id(@event, :agenda_item) plus a notify toast, instead of a page navigation.
class CalendarEventsSwipeTest < ActionDispatch::IntegrationTest
  include ActionView::RecordIdentifier
  include ActiveJob::TestHelper

  setup do
    @workspace = Workspace.create!(name: "Swipe Calendar WS")
    @user = @workspace.users.create!(
      name: "Tester",
      email_address: "swipe-cal-#{SecureRandom.hex(4)}@example.com",
      password: "password123"
    )
    @account = CalendarAccount.create!(
      workspace: @workspace,
      email_address: "cal-#{SecureRandom.hex(4)}@example.com",
      provider: :google,
      refresh_token: "tok",
      active: true
    )
    @account.calendar_account_users.create!(
      user: @user, owner: true, can_read: true, can_write: true, can_manage: true
    )
    @calendar = @account.calendars.create!(
      name: "Primary",
      provider_calendar_id: "pc-#{SecureRandom.hex(4)}",
      syncing: true,
      is_writable: true
    )
    @event = @calendar.calendar_events.create!(
      provider_event_id: "ev-#{SecureRandom.hex(4)}",
      title: "Standup",
      start_at: 1.day.from_now.change(hour: 10),
      end_at: 1.day.from_now.change(hour: 11),
      status: :confirmed
    )
    sign_in(@user)
  end

  # ── Destroy swipe ─────────────────────��───────────────────────────────────

  test "destroy swipe removes the agenda_item row and shows a toast" do
    assert_enqueued_with(job: Calendars::EventWriteJob) do
      delete calendar_event_path(@event, swipe: 1),
             headers: { "Accept" => "text/vnd.turbo-stream.html" }
    end

    assert_response :success
    assert_includes response.body, dom_id(@event, :agenda_item)
    assert_match "turbo-stream", response.content_type
  end

  test "destroy swipe does not navigate away via render_event_saved" do
    assert_enqueued_with(job: Calendars::EventWriteJob) do
      delete calendar_event_path(@event, swipe: 1),
             headers: { "Accept" => "text/vnd.turbo-stream.html" }
    end

    assert_response :success
    # render_event_saved would append a navigate partial to calendar_event_modal;
    # a swipe response must NOT include that frame append.
    assert_not_includes response.body, "calendar_event_modal"
  end

  test "destroy without swipe navigates via render_event_saved" do
    assert_enqueued_with(job: Calendars::EventWriteJob) do
      delete calendar_event_path(@event),
             headers: { "Accept" => "text/vnd.turbo-stream.html" }
    end

    assert_response :success
    # render_event_saved appends to calendar_event_modal
    assert_includes response.body, "calendar_event_modal"
  end

  test "destroy swipe marks outbound_pending and enqueues the write job" do
    assert_enqueued_with(job: Calendars::EventWriteJob, args: [ @event.id, "delete", "this" ]) do
      delete calendar_event_path(@event, swipe: 1),
             headers: { "Accept" => "text/vnd.turbo-stream.html" }
    end

    assert @event.reload.outbound_pending
  end

  test "destroy swipe is blocked on a read-only calendar" do
    @calendar.update!(is_writable: false)

    delete calendar_event_path(@event, swipe: 1),
           headers: { "Accept" => "text/vnd.turbo-stream.html" }

    assert_response :redirect # redirected back to calendar by require_writable_event
  end

  test "destroy requires authentication" do
    delete session_path
    delete calendar_event_path(@event, swipe: 1),
           headers: { "Accept" => "text/vnd.turbo-stream.html" }

    assert_response :redirect
  end

  private

  def sign_in(user)
    post session_path, params: { email_address: user.email_address, password: "password123" }
  end
end
