# frozen_string_literal: true

require "rails_helper"

RSpec.describe "Calendar Import API (app)", type: :request do
  let(:workspace)        { create(:workspace) }
  let(:user)             { create(:user, workspace: workspace) }
  let(:calendar_account) do
    create(:calendar_account, workspace: workspace).tap do |acc|
      create(:calendar_account_user, calendar_account: acc, user: user,
                                     can_read: true, can_write: true, can_manage: true, owner: true)
    end
  end
  let!(:calendar) { create(:calendar, calendar_account: calendar_account, syncing: true, is_writable: true) }

  # A minimal valid ICS string (one non-recurring event).
  let(:ics_content) do
    <<~ICS
      BEGIN:VCALENDAR
      VERSION:2.0
      PRODID:-//Test//EN
      BEGIN:VEVENT
      UID:test-event-1@example.com
      DTSTART:20261001T100000Z
      DTEND:20261001T110000Z
      SUMMARY:Imported event
      END:VEVENT
      END:VCALENDAR
    ICS
  end

  around do |ex|
    travel_to ::Time.zone.local(2026, 9, 20, 12, 0, 0), &ex
  end

  before do
    allow(Calendars::EventWriteJob).to receive(:perform_later)
  end

  # Build an UploadedFile from a string using a Tempfile so Rack processes it correctly.
  def ics_upload(content, filename: "events.ics")
    tmp = Tempfile.new([ "import_spec", ".ics" ])
    tmp.write(content)
    tmp.rewind
    Rack::Test::UploadedFile.new(tmp.path, "text/calendar", original_filename: filename)
  end

  describe "POST /api/app/calendar_import" do
    it "imports events and returns a result summary" do
      post "/api/app/calendar_import",
           params: { calendar_id: calendar.id, ics_file: ics_upload(ics_content) },
           headers: api_app_headers(user)

      expect(response).to have_http_status(:created)
      data = response.parsed_body["data"]
      expect(data["imported"]).to eq(1)
      expect(data["skipped_recurring"]).to eq(0)
      expect(data["skipped_duplicate"]).to eq(0)
    end

    it "skips duplicates on a second import of the same file" do
      post "/api/app/calendar_import",
           params: { calendar_id: calendar.id, ics_file: ics_upload(ics_content) },
           headers: api_app_headers(user)
      post "/api/app/calendar_import",
           params: { calendar_id: calendar.id, ics_file: ics_upload(ics_content) },
           headers: api_app_headers(user)

      second = response.parsed_body["data"]
      expect(second["imported"]).to eq(0)
      expect(second["skipped_duplicate"]).to eq(1)
    end

    it "422s when no calendar_id is given" do
      post "/api/app/calendar_import",
           params: { ics_file: ics_upload(ics_content) },
           headers: api_app_headers(user)

      expect(response).to have_http_status(:unprocessable_entity)
    end

    it "422s when the calendar belongs to another workspace" do
      other_ws      = create(:workspace)
      other_account = create(:calendar_account, workspace: other_ws)
      other_cal     = create(:calendar, calendar_account: other_account, syncing: true, is_writable: true)

      post "/api/app/calendar_import",
           params: { calendar_id: other_cal.id, ics_file: ics_upload(ics_content) },
           headers: api_app_headers(user)

      expect(response).to have_http_status(:unprocessable_entity)
    end

    it "422s when no file is provided" do
      post "/api/app/calendar_import",
           params: { calendar_id: calendar.id },
           headers: api_app_headers(user)

      expect(response).to have_http_status(:unprocessable_entity)
    end

    it "401s without a token" do
      post "/api/app/calendar_import", params: { calendar_id: calendar.id }
      expect(response).to have_http_status(:unauthorized)
    end
  end
end
