require "test_helper"

# The calendar-management sidebar surfaces: per-user show/hide
# (CalendarVisibilitiesController), the on-demand provider list refresh
# (CalendarAccountsController#refresh), and the CalendarController#index
# filtering they drive (only syncing calendars render, minus hidden ones).
class CalendarSidebarTest < ActionDispatch::IntegrationTest
  include ActiveJob::TestHelper

  setup do
    @workspace = Workspace.create!(name: "Calendar Sidebar WS")
    @user = create_user("owner")
    @account = create_calendar_account(@user)
    sign_in(@user)
  end

  # ── Visibility toggle ──────────────────────────────────────────────────────

  test "hiding a calendar removes its events from the calendar page" do
    shown = create_calendar(@account, name: "Kept")
    hidden = create_calendar(@account, name: "Muted")
    create_event(shown, title: "Visible standup")
    create_event(hidden, title: "Hidden retro")

    patch calendar_visibility_path(hidden), params: { hidden: "1", view: "month", date: Date.current.iso8601 }

    assert_redirected_to calendar_path(view: "month", date: Date.current.iso8601)
    assert_includes @user.reload.hidden_calendar_ids, hidden.id.to_s

    get calendar_path(view: "month")
    assert_response :success
    assert_match "Visible standup", response.body
    assert_no_match "Hidden retro", response.body
  end

  test "the toggle is idempotent and reversible" do
    calendar = create_calendar(@account)

    2.times { patch calendar_visibility_path(calendar), params: { hidden: "1" } }
    assert_equal [ calendar.id.to_s ], @user.reload.hidden_calendar_ids

    patch calendar_visibility_path(calendar), params: { hidden: "0" }
    assert_empty @user.reload.hidden_calendar_ids
  end

  test "hiding is per-user, not account-wide" do
    calendar = create_calendar(@account)
    other = create_user("other")
    @account.calendar_account_users.create!(user: other, can_read: true)

    patch calendar_visibility_path(calendar), params: { hidden: "1" }

    assert other.reload.hidden_calendar_ids.empty?
  end

  test "a calendar on an unshared account is not found" do
    stranger = create_user("stranger")
    foreign_account = create_calendar_account(stranger)
    foreign_calendar = create_calendar(foreign_account)

    patch calendar_visibility_path(foreign_calendar), params: { hidden: "1" }

    assert_response :not_found
  end

  # ── Index filtering ────────────────────────────────────────────────────────

  test "events from calendars that stopped syncing no longer render" do
    active = create_calendar(@account, name: "Active")
    paused = create_calendar(@account, name: "Paused", syncing: false)
    create_event(active, title: "Active planning")
    create_event(paused, title: "Stale kickoff")

    get calendar_path(view: "month")

    assert_response :success
    assert_match "Active planning", response.body
    assert_no_match "Stale kickoff", response.body
  end

  test "the sidebar lists the account with its synced and not-yet-synced calendars" do
    create_calendar(@account, name: "Team calendar")
    create_calendar(@account, name: "Holidays", syncing: false)

    get calendar_path

    assert_response :success
    assert_match @account.email_address, response.body
    assert_match "Team calendar", response.body
    assert_match "Holidays", response.body
  end

  # ── Provider list refresh ──────────────────────────────────────────────────

  test "refresh enqueues a full scan for a manager" do
    assert_enqueued_with(job: CalendarScanJob, args: [ @account.id, "full" ]) do
      post refresh_calendar_account_path(@account)
    end
    assert_response :redirect
  end

  test "refresh is denied for a viewer" do
    viewer = create_user("viewer")
    @account.calendar_account_users.create!(user: viewer, can_read: true)
    delete session_path # drop the owner session from setup
    sign_in(viewer)

    assert_no_enqueued_jobs(only: CalendarScanJob) do
      post refresh_calendar_account_path(@account)
    end
    assert_redirected_to calendar_path
  end

  private

  def create_user(prefix)
    @workspace.users.create!(
      name: prefix.capitalize,
      email_address: "#{prefix}-#{SecureRandom.hex(4)}@example.com",
      password: "password123"
    )
  end

  def sign_in(user)
    post session_path, params: { email_address: user.email_address, password: "password123" }
  end

  def create_calendar_account(owner)
    account = CalendarAccount.create!(
      workspace: @workspace, email_address: "cal-#{SecureRandom.hex(4)}@example.com",
      provider: :google, refresh_token: "tok", active: true
    )
    account.calendar_account_users.create!(user: owner, owner: true, can_read: true, can_write: true, can_manage: true)
    account
  end

  def create_calendar(account, name: "Calendar #{SecureRandom.hex(2)}", syncing: true)
    account.calendars.create!(
      name: name, provider_calendar_id: "pc-#{SecureRandom.hex(4)}",
      syncing: syncing, is_writable: true
    )
  end

  def create_event(calendar, title:)
    calendar.calendar_events.create!(
      provider_event_id: "ev-#{SecureRandom.hex(4)}", title: title,
      start_at: Time.current.change(hour: 10), end_at: Time.current.change(hour: 11),
      status: :confirmed
    )
  end
end
