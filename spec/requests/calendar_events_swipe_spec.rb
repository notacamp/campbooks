# frozen_string_literal: true

require "rails_helper"

# Verifies the turbo_stream swipe branch added to CalendarEventsController#destroy.
# A swipe request sends params[:swipe]=1 and expects turbo_stream.remove of
# dom_id(@event, :agenda_item) plus a notify toast, instead of a page navigation.
RSpec.describe "Calendar events swipe actions", type: :request do
  include ActionView::RecordIdentifier
  include ActiveJob::TestHelper

  before do
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

  # ── Destroy swipe ─────────────────────────────────────────────────────────

  it "destroy swipe removes the agenda_item row and shows a toast" do
    expect {
      delete calendar_event_path(@event, swipe: 1),
             headers: { "Accept" => "text/vnd.turbo-stream.html" }
    }.to have_enqueued_job(Calendars::EventWriteJob)

    expect(response).to have_http_status(:ok)
    expect(response.body).to include(dom_id(@event, :agenda_item))
    expect(response.content_type).to match("turbo-stream")
  end

  it "destroy swipe does not navigate away via render_event_saved" do
    expect {
      delete calendar_event_path(@event, swipe: 1),
             headers: { "Accept" => "text/vnd.turbo-stream.html" }
    }.to have_enqueued_job(Calendars::EventWriteJob)

    expect(response).to have_http_status(:ok)
    # render_event_saved would append a navigate partial to calendar_event_modal;
    # a swipe response must NOT include that frame append.
    expect(response.body).not_to include("calendar_event_modal")
  end

  it "destroy without swipe navigates via render_event_saved" do
    expect {
      delete calendar_event_path(@event),
             headers: { "Accept" => "text/vnd.turbo-stream.html" }
    }.to have_enqueued_job(Calendars::EventWriteJob)

    expect(response).to have_http_status(:ok)
    # render_event_saved appends to calendar_event_modal
    expect(response.body).to include("calendar_event_modal")
  end

  it "destroy swipe marks outbound_pending and enqueues the write job" do
    expect {
      delete calendar_event_path(@event, swipe: 1),
             headers: { "Accept" => "text/vnd.turbo-stream.html" }
    }.to have_enqueued_job(Calendars::EventWriteJob).with(@event.id, "delete", "this")

    expect(@event.reload.outbound_pending).to be_truthy
  end

  it "destroy swipe is blocked on a read-only calendar" do
    @calendar.update!(is_writable: false)

    delete calendar_event_path(@event, swipe: 1),
           headers: { "Accept" => "text/vnd.turbo-stream.html" }

    # redirected back to calendar by require_writable_event
    expect(response).to have_http_status(:found)
  end

  it "destroy requires authentication" do
    delete session_path
    delete calendar_event_path(@event, swipe: 1),
           headers: { "Accept" => "text/vnd.turbo-stream.html" }

    expect(response).to have_http_status(:found)
  end
end
