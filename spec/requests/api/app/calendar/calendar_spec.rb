# frozen_string_literal: true

require "rails_helper"

RSpec.describe "GET /api/app/calendar", type: :request do
  let(:workspace)        { create(:workspace) }
  let(:user)             { create(:user, workspace: workspace) }
  let(:calendar_account) { create(:calendar_account, workspace: workspace) }
  let!(:account_user) do
    create(:calendar_account_user, calendar_account: calendar_account, user: user,
                                   can_read: true, owner: true, can_manage: true)
  end
  let!(:calendar) { create(:calendar, calendar_account: calendar_account, syncing: true) }

  around do |ex|
    travel_to ::Time.zone.local(2026, 9, 20, 12, 0, 0), &ex
  end

  context "when authenticated" do
    it "returns 200 with the screen payload for the default view" do
      get "/api/app/calendar", headers: api_app_headers(user)

      expect(response).to have_http_status(:ok)
      data = response.parsed_body["data"]
      expect(data).to include("view", "date", "range", "has_calendars", "calendar_accounts", "events", "reminders")
    end

    it "returns the correct view and date when specified" do
      get "/api/app/calendar", params: { view: "week", date: "2026-09-14" },
                               headers: api_app_headers(user)

      expect(response).to have_http_status(:ok)
      data = response.parsed_body["data"]
      expect(data["view"]).to eq("week")
      expect(data["date"]).to eq("2026-09-14")
    end

    it "includes the calendar account in the sidebar data" do
      get "/api/app/calendar", headers: api_app_headers(user)

      data = response.parsed_body["data"]
      expect(data["has_calendars"]).to be(true)
      expect(data["calendar_accounts"].length).to be >= 1
      expect(data["calendar_accounts"].first["id"]).to eq(calendar_account.id)
    end

    it "returns events visible to the user" do
      event = create(:calendar_event, calendar: calendar)

      get "/api/app/calendar", params: { view: "month", date: event.start_at.to_date.iso8601 },
                               headers: api_app_headers(user)

      data = response.parsed_body["data"]
      event_ids = data["events"].map { |e| e["id"] }
      expect(event_ids).to include(event.id)
    end

    it "coerces an unknown view to month" do
      get "/api/app/calendar", params: { view: "invalid" }, headers: api_app_headers(user)

      data = response.parsed_body["data"]
      expect(data["view"]).to eq("month")
    end

    it "does not expose events from another workspace" do
      other_workspace = create(:workspace)
      other_user      = create(:user, workspace: other_workspace)
      other_account   = create(:calendar_account, workspace: other_workspace)
      create(:calendar_account_user, calendar_account: other_account, user: other_user,
                                     can_read: true, owner: true)
      other_calendar  = create(:calendar, calendar_account: other_account, syncing: true)
      other_event     = create(:calendar_event, calendar: other_calendar)

      get "/api/app/calendar", params: { view: "month", date: other_event.start_at.to_date.iso8601 },
                               headers: api_app_headers(user)

      data = response.parsed_body["data"]
      event_ids = data["events"].map { |e| e["id"] }
      expect(event_ids).not_to include(other_event.id)
    end

    it "includes prev_date and next_date for navigation" do
      get "/api/app/calendar", params: { view: "day", date: "2026-09-20" },
                               headers: api_app_headers(user)

      data = response.parsed_body["data"]
      expect(data["prev_date"]).to eq("2026-09-19")
      expect(data["next_date"]).to eq("2026-09-21")
    end
  end

  context "without authentication" do
    it "returns 401" do
      get "/api/app/calendar"
      expect(response).to have_http_status(:unauthorized)
    end
  end
end
