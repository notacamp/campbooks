require "rails_helper"

RSpec.describe "CalendarImports", type: :request do
  include ActiveJob::TestHelper

  before do
    @workspace = Workspace.create!(name: "ICS Import WS")
    @user = @workspace.users.create!(name: "Owner", email_address: "owner-#{SecureRandom.hex(4)}@example.com",
                                     password: "password123")
    @account = CalendarAccount.create!(workspace: @workspace, email_address: "cal-#{SecureRandom.hex(4)}@example.com",
                                       provider: :google, refresh_token: "tok", active: true)
    @account.calendar_account_users.create!(user: @user, owner: true, can_read: true, can_write: true, can_manage: true)
    @calendar = @account.calendars.create!(name: "Primary", provider_calendar_id: "pc-1",
                                           syncing: true, is_writable: true)
    sign_in(@user)
  end

  it "requires authentication" do
    delete session_path
    post calendar_import_path, params: { calendar_id: @calendar.id }

    expect(response).to have_http_status(:found)
    # bounced to sign-in, not the calendar
    expect(response.location).not_to match("calendar")
  end

  it "imports the fixture file and reports counts" do
    expect {
      expect {
        post calendar_import_path, params: {
          calendar_id: @calendar.id,
          ics_file: Rack::Test::UploadedFile.new(
            Rails.root.join("spec/fixtures/files/sample_import.ics"), "text/calendar"
          )
        }
      }.to have_enqueued_job(Calendars::EventWriteJob).exactly(2).times
    }.to change { @calendar.calendar_events.count }.by(2)

    expect(response).to redirect_to(calendar_path)
    follow_redirect!
    expect(response.body).to include("2 events imported")
    expect(response.body).to include("1 recurring event skipped")

    timed = @calendar.calendar_events.find_by(ics_uid: "fixture-1@example.com")
    expect(timed.title).to eq("Quarterly review")
    expect(timed.outbound_pending).to be_truthy
  end

  it "rejects a non-calendar file" do
    expect {
      post calendar_import_path, params: {
        calendar_id: @calendar.id,
        ics_file: Rack::Test::UploadedFile.new(
          Rails.root.join("spec/fixtures/files/not_a_calendar.txt"), "text/plain"
        )
      }
    }.not_to change { CalendarEvent.count }

    expect(response).to redirect_to(new_calendar_import_path)
  end

  it "rejects an oversized file" do
    big = Tempfile.new([ "huge", ".ics" ])
    big.write("A" * (CalendarImportsController::MAX_FILE_BYTES + 1))
    big.rewind

    expect {
      post calendar_import_path, params: {
        calendar_id: @calendar.id,
        ics_file: Rack::Test::UploadedFile.new(big.path, "text/calendar")
      }
    }.not_to change { CalendarEvent.count }

    expect(response).to redirect_to(new_calendar_import_path)
  ensure
    big.close!
  end

  it "rejects a calendar the user cannot write to" do
    read_only = @account.calendars.create!(name: "Read only", provider_calendar_id: "pc-ro",
                                           syncing: true, is_writable: false)

    expect {
      post calendar_import_path, params: {
        calendar_id: read_only.id,
        ics_file: Rack::Test::UploadedFile.new(
          Rails.root.join("spec/fixtures/files/sample_import.ics"), "text/calendar"
        )
      }
    }.not_to change { CalendarEvent.count }

    expect(response).to redirect_to(new_calendar_import_path)
  end

  it "new bounces to the calendar when there is no writable calendar" do
    @calendar.update!(is_writable: false)
    get new_calendar_import_path

    expect(response).to redirect_to(calendar_path)
  end
end
