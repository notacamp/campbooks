# frozen_string_literal: true

require "rails_helper"

RSpec.describe "Calendar Events API (app)", type: :request do
  let(:workspace)        { create(:workspace) }
  let(:user)             { create(:user, workspace: workspace) }
  let(:calendar_account) do
    create(:calendar_account, workspace: workspace).tap do |acc|
      create(:calendar_account_user, calendar_account: acc, user: user,
                                     can_read: true, can_write: true, can_manage: true, owner: true)
    end
  end
  let(:calendar) { create(:calendar, calendar_account: calendar_account, syncing: true, is_writable: true) }

  around do |ex|
    travel_to ::Time.zone.local(2026, 9, 20, 12, 0, 0), &ex
  end

  before do
    # Prevent live provider writes in all write tests
    allow(Calendars::EventWriteJob).to receive(:perform_later)
  end

  # ── GET /api/app/calendar_events/:id ──────────────────────────────────────────

  describe "GET /api/app/calendar_events/:id" do
    let(:event) { create(:calendar_event, calendar: calendar) }

    it "returns the event detail" do
      get "/api/app/calendar_events/#{event.id}", headers: api_app_headers(user)

      expect(response).to have_http_status(:ok)
      data = response.parsed_body["data"]
      expect(data["id"]).to eq(event.id)
      expect(data).to include("description", "attendees", "rrule")
    end

    it "404s for an event in another workspace" do
      other_ws      = create(:workspace)
      other_user    = create(:user, workspace: other_ws)
      other_account = create(:calendar_account, workspace: other_ws).tap do |acc|
        create(:calendar_account_user, calendar_account: acc, user: other_user,
                                       can_read: true, owner: true)
      end
      other_cal     = create(:calendar, calendar_account: other_account)
      other_event   = create(:calendar_event, calendar: other_cal)

      get "/api/app/calendar_events/#{other_event.id}", headers: api_app_headers(user)

      expect(response).to have_http_status(:not_found)
    end
  end

  # ── POST /api/app/calendar_events ─────────────────────────────────────────────

  describe "POST /api/app/calendar_events" do
    let(:valid_params) do
      {
        calendar_id: calendar.id,
        title: "Team sync",
        start_at: "2026-09-21T10:00:00Z",
        end_at: "2026-09-21T11:00:00Z"
      }
    end

    it "creates the event and enqueues the write job" do
      post "/api/app/calendar_events", params: valid_params, headers: api_app_headers(user)

      expect(response).to have_http_status(:created)
      data = response.parsed_body["data"]
      expect(data["title"]).to eq("Team sync")
      expect(data["outbound_pending"]).to be(true)
      expect(Calendars::EventWriteJob).to have_received(:perform_later).once
    end

    it "403s when the calendar is not writable" do
      read_only_calendar = create(:calendar, :read_only, calendar_account: calendar_account, syncing: true)

      post "/api/app/calendar_events",
           params: valid_params.merge(calendar_id: read_only_calendar.id),
           headers: api_app_headers(user)

      expect(response).to have_http_status(:forbidden)
      expect(Calendars::EventWriteJob).not_to have_received(:perform_later)
    end

    it "403s when the calendar belongs to another workspace" do
      other_ws      = create(:workspace)
      other_account = create(:calendar_account, workspace: other_ws)
      other_cal     = create(:calendar, calendar_account: other_account, syncing: true, is_writable: true)

      post "/api/app/calendar_events",
           params: valid_params.merge(calendar_id: other_cal.id),
           headers: api_app_headers(user)

      expect(response).to have_http_status(:forbidden)
    end
  end

  # ── PATCH /api/app/calendar_events/:id ────────────────────────────────────────

  describe "PATCH /api/app/calendar_events/:id" do
    let(:event) { create(:calendar_event, calendar: calendar, is_organizer: true) }

    it "updates the event and enqueues the write job" do
      patch "/api/app/calendar_events/#{event.id}",
            params: { title: "Updated title" },
            headers: api_app_headers(user)

      expect(response).to have_http_status(:ok)
      expect(response.parsed_body.dig("data", "title")).to eq("Updated title")
      expect(Calendars::EventWriteJob).to have_received(:perform_later).once
    end

    it "uses recurrence_scope=this by default" do
      patch "/api/app/calendar_events/#{event.id}",
            params: { title: "Updated" },
            headers: api_app_headers(user)

      expect(Calendars::EventWriteJob).to have_received(:perform_later).with(event.id, "update", "this")
    end

    it "respects recurrence_scope=all" do
      patch "/api/app/calendar_events/#{event.id}",
            params: { title: "Updated", recurrence_scope: "all" },
            headers: api_app_headers(user)

      expect(Calendars::EventWriteJob).to have_received(:perform_later).with(event.id, "update", "all")
    end
  end

  # ── DELETE /api/app/calendar_events/:id ───────────────────────────────────────

  describe "DELETE /api/app/calendar_events/:id" do
    let(:event) { create(:calendar_event, calendar: calendar) }

    it "marks outbound_pending and enqueues a delete job, returns 202" do
      delete "/api/app/calendar_events/#{event.id}", headers: api_app_headers(user)

      expect(response).to have_http_status(:accepted)
      expect(response.parsed_body.dig("data", "outbound_pending")).to be(true)
      expect(Calendars::EventWriteJob).to have_received(:perform_later).with(event.id, "delete", anything)
    end
  end

  # ── POST /api/app/calendar_events/:id/rsvp ────────────────────────────────────

  describe "POST /api/app/calendar_events/:id/rsvp" do
    let(:event) { create(:calendar_event, calendar: calendar) }

    it "updates rsvp_status and enqueues the write job" do
      post "/api/app/calendar_events/#{event.id}/rsvp",
           params: { rsvp_status: "accepted" },
           headers: api_app_headers(user)

      expect(response).to have_http_status(:ok)
      expect(response.parsed_body.dig("data", "rsvp_status")).to eq("accepted")
      expect(Calendars::EventWriteJob).to have_received(:perform_later).with(event.id, "rsvp")
    end

    it "422s for an invalid rsvp_status" do
      post "/api/app/calendar_events/#{event.id}/rsvp",
           params: { rsvp_status: "bogus" },
           headers: api_app_headers(user)

      expect(response).to have_http_status(:unprocessable_entity)
    end
  end

  # ── PATCH /api/app/calendar_events/:id/reschedule ─────────────────────────────

  describe "PATCH /api/app/calendar_events/:id/reschedule" do
    let(:event) { create(:calendar_event, calendar: calendar) }

    it "updates start_at/end_at and enqueues a write job" do
      patch "/api/app/calendar_events/#{event.id}/reschedule",
            params: { start_at: "2026-09-22T09:00:00Z", end_at: "2026-09-22T10:00:00Z" },
            headers: api_app_headers(user)

      expect(response).to have_http_status(:ok)
      expect(Calendars::EventWriteJob).to have_received(:perform_later).with(event.id, "update", "this")
    end

    it "403s when the event is not writable by the user" do
      unwritable_account = create(:calendar_account, workspace: workspace)
      create(:calendar_account_user, calendar_account: unwritable_account, user: user,
                                     can_read: true, can_write: false)
      unwritable_cal = create(:calendar, :read_only, calendar_account: unwritable_account, syncing: true)
      unwritable_event = create(:calendar_event, calendar: unwritable_cal)

      patch "/api/app/calendar_events/#{unwritable_event.id}/reschedule",
            params: { start_at: "2026-09-22T09:00:00Z", end_at: "2026-09-22T10:00:00Z" },
            headers: api_app_headers(user)

      expect(response).to have_http_status(:forbidden)
    end
  end

  context "without authentication" do
    it "401s on show" do
      event = create(:calendar_event, calendar: calendar)
      get "/api/app/calendar_events/#{event.id}"
      expect(response).to have_http_status(:unauthorized)
    end
  end
end
