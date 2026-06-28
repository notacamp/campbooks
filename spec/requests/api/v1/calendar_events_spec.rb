require "rails_helper"

RSpec.describe "API v1 calendar events", type: :request do
  include ActiveJob::TestHelper

  let(:workspace)        { create(:workspace) }
  let(:user)             { create(:user, workspace: workspace) }
  let(:calendar_account) { create(:calendar_account, workspace: workspace) }
  let(:calendar)         { create(:calendar, calendar_account: calendar_account) }

  # Give the user read+write access to the calendar account (editor = can_read + can_write).
  before do
    create(:calendar_account_user, :editor, calendar_account: calendar_account, user: user)
  end

  def read_headers
    api_auth_headers(workspace: workspace, user: user, scopes: "calendar:read")
  end

  def write_headers
    api_auth_headers(workspace: workspace, user: user, scopes: "calendar:write")
  end

  describe "GET /api/v1/calendar_events" do
    it "lists accessible events ordered by start_at ascending" do
      later  = create(:calendar_event, calendar: calendar, title: "Later",
                                       start_at: 2.days.from_now, end_at: 2.days.from_now + 1.hour)
      sooner = create(:calendar_event, calendar: calendar, title: "Sooner",
                                       start_at: 1.day.from_now,  end_at: 1.day.from_now + 1.hour)

      get api_v1_calendar_events_path, headers: read_headers

      expect(response).to have_http_status(:ok)
      titles = response.parsed_body["data"].map { |e| e["title"] }
      expect(titles).to eq([ sooner.title, later.title ])
    end

    it "filters by start_after" do
      create(:calendar_event, calendar: calendar,
                               start_at: 1.day.from_now, end_at: 1.day.from_now + 1.hour)
      future = create(:calendar_event, calendar: calendar,
                                       start_at: 3.days.from_now, end_at: 3.days.from_now + 1.hour)

      get api_v1_calendar_events_path, params: { start_after: 2.days.from_now.iso8601 },
                                       headers: read_headers

      ids = response.parsed_body["data"].map { |e| e["id"] }
      expect(ids).to eq([ future.id ])
    end

    it "does not leak another workspace's events" do
      other_ws      = create(:workspace)
      other_account = create(:calendar_account, workspace: other_ws)
      other_cal     = create(:calendar, calendar_account: other_account)
      create(:calendar_event, calendar: other_cal)

      get api_v1_calendar_events_path, headers: read_headers

      expect(response.parsed_body["data"]).to be_empty
    end
  end

  describe "GET /api/v1/calendar_events/:id" do
    it "404s for an event in another workspace" do
      other_ws      = create(:workspace)
      other_account = create(:calendar_account, workspace: other_ws)
      other_cal     = create(:calendar, calendar_account: other_account)
      event         = create(:calendar_event, calendar: other_cal)

      get api_v1_calendar_event_path(event), headers: read_headers

      expect(response).to have_http_status(:not_found)
    end

    it "returns event detail when accessible" do
      event = create(:calendar_event, calendar: calendar)

      get api_v1_calendar_event_path(event), headers: read_headers

      expect(response).to have_http_status(:ok)
      expect(response.parsed_body.dig("data", "id")).to eq(event.id)
      expect(response.parsed_body["data"]).to include("description", "attendees", "rrule")
    end
  end

  describe "POST /api/v1/calendar_events" do
    let(:event_attrs) do
      {
        calendar_id: calendar.id,
        title: "Team Sync",
        start_at: 1.day.from_now.iso8601,
        end_at: (1.day.from_now + 1.hour).iso8601
      }
    end

    it "creates the event locally and enqueues an EventWriteJob" do
      expect do
        post api_v1_calendar_events_path, params: event_attrs, headers: write_headers
      end.to change(CalendarEvent, :count).by(1)
              .and have_enqueued_job(Calendars::EventWriteJob)

      expect(response).to have_http_status(:created)
      event = CalendarEvent.last
      expect(event.outbound_pending).to be(true)
      expect(event.status).to eq("confirmed")
    end

    it "403s when the target calendar is not in the user's writable set" do
      other_ws      = create(:workspace)
      other_account = create(:calendar_account, workspace: other_ws)
      other_cal     = create(:calendar, calendar_account: other_account)

      post api_v1_calendar_events_path,
           params: event_attrs.merge(calendar_id: other_cal.id),
           headers: write_headers

      expect(response).to have_http_status(:forbidden)
      expect(response.parsed_body.dig("error", "code")).to eq("calendar_not_writable")
    end

    it "403s when only the read scope is present" do
      post api_v1_calendar_events_path, params: event_attrs, headers: read_headers
      expect(response).to have_http_status(:forbidden)
    end
  end

  describe "PATCH /api/v1/calendar_events/:id" do
    it "updates the event and enqueues an update job" do
      event = create(:calendar_event, calendar: calendar)

      expect do
        patch api_v1_calendar_event_path(event),
              params: { title: "Renamed", recurrence_scope: "this" },
              headers: write_headers
      end.to have_enqueued_job(Calendars::EventWriteJob).with(event.id, "update", "this")

      expect(response).to have_http_status(:ok)
      expect(event.reload.outbound_pending).to be(true)
      expect(event.reload.title).to eq("Renamed")
    end
  end

  describe "DELETE /api/v1/calendar_events/:id" do
    it "marks outbound_pending and enqueues a delete job, returning 202" do
      event = create(:calendar_event, calendar: calendar)

      expect do
        delete api_v1_calendar_event_path(event), headers: write_headers
      end.to have_enqueued_job(Calendars::EventWriteJob).with(event.id, "delete", "this")

      expect(response).to have_http_status(:accepted)
      expect(event.reload.outbound_pending).to be(true)
    end
  end

  describe "POST /api/v1/calendar_events/:id/rsvp" do
    it "sets rsvp_status and enqueues an rsvp job" do
      event = create(:calendar_event, calendar: calendar)

      expect do
        post rsvp_api_v1_calendar_event_path(event),
             params: { rsvp_status: "accepted" },
             headers: write_headers
      end.to have_enqueued_job(Calendars::EventWriteJob).with(event.id, "rsvp")

      expect(response).to have_http_status(:ok)
      expect(event.reload.rsvp_status).to eq("accepted")
      expect(event.reload.outbound_pending).to be(true)
    end

    it "422s when rsvp_status is not a known value" do
      event = create(:calendar_event, calendar: calendar)

      post rsvp_api_v1_calendar_event_path(event),
           params: { rsvp_status: "bogus" },
           headers: write_headers

      expect(response).to have_http_status(:unprocessable_entity)
      expect(response.parsed_body.dig("error", "code")).to eq("invalid_rsvp_status")
    end
  end
end
