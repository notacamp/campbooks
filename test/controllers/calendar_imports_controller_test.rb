require "test_helper"

class CalendarImportsControllerTest < ActionDispatch::IntegrationTest
  include ActiveJob::TestHelper

  setup do
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

  test "requires authentication" do
    delete session_path
    post calendar_import_path, params: { calendar_id: @calendar.id }
    assert_response :redirect
    assert_no_match "calendar", response.location.to_s # bounced to sign-in, not the calendar
  end

  test "imports the fixture file and reports counts" do
    assert_difference -> { @calendar.calendar_events.count }, 2 do
      assert_enqueued_jobs 2, only: Calendars::EventWriteJob do
        post calendar_import_path, params: {
          calendar_id: @calendar.id,
          ics_file: fixture_file_upload("sample_import.ics", "text/calendar")
        }
      end
    end

    assert_redirected_to calendar_path
    follow_redirect!
    assert_match "2 events imported", response.body
    assert_match "1 recurring event skipped", response.body

    timed = @calendar.calendar_events.find_by(ics_uid: "fixture-1@example.com")
    assert_equal "Quarterly review", timed.title
    assert timed.outbound_pending
  end

  test "rejects a non-calendar file" do
    assert_no_difference -> { CalendarEvent.count } do
      post calendar_import_path, params: {
        calendar_id: @calendar.id,
        ics_file: fixture_file_upload("not_a_calendar.txt", "text/plain")
      }
    end
    assert_redirected_to new_calendar_import_path
  end

  test "rejects an oversized file" do
    big = Tempfile.new([ "huge", ".ics" ])
    big.write("A" * (CalendarImportsController::MAX_FILE_BYTES + 1))
    big.rewind

    assert_no_difference -> { CalendarEvent.count } do
      post calendar_import_path, params: {
        calendar_id: @calendar.id,
        ics_file: Rack::Test::UploadedFile.new(big.path, "text/calendar")
      }
    end
    assert_redirected_to new_calendar_import_path
  ensure
    big.close!
  end

  test "rejects a calendar the user cannot write to" do
    read_only = @account.calendars.create!(name: "Read only", provider_calendar_id: "pc-ro",
                                           syncing: true, is_writable: false)

    assert_no_difference -> { CalendarEvent.count } do
      post calendar_import_path, params: {
        calendar_id: read_only.id,
        ics_file: fixture_file_upload("sample_import.ics", "text/calendar")
      }
    end
    assert_redirected_to new_calendar_import_path
  end

  test "new bounces to the calendar when there is no writable calendar" do
    @calendar.update!(is_writable: false)
    get new_calendar_import_path
    assert_redirected_to calendar_path
  end

  private

  def sign_in(user)
    post session_path, params: { email_address: user.email_address, password: "password123" }
  end
end
