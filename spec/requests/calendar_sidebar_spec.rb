require "rails_helper"

# The calendar-management sidebar surfaces: per-user show/hide
# (CalendarVisibilitiesController), the on-demand provider list refresh
# (CalendarAccountsController#refresh), and the CalendarController#index
# filtering they drive (only syncing calendars render, minus hidden ones).
RSpec.describe "Calendar sidebar", type: :request do
  include ActiveJob::TestHelper

  before do
    @workspace = Workspace.create!(name: "Calendar Sidebar WS")
    @user = create_user("owner")
    @account = create_calendar_account(@user)
    sign_in(@user)
  end

  # ── Visibility toggle ──────────────────────────────────────────────────────

  it "hiding a calendar removes its events from the calendar page" do
    shown  = create_calendar(@account, name: "Kept")
    hidden = create_calendar(@account, name: "Muted")
    create_event(shown,  title: "Visible standup")
    create_event(hidden, title: "Hidden retro")

    patch calendar_visibility_path(hidden), params: { hidden: "1", view: "month", date: Date.current.iso8601 }

    expect(response).to redirect_to(calendar_path(view: "month", date: Date.current.iso8601))
    expect(@user.reload.hidden_calendar_ids).to include(hidden.id.to_s)

    get calendar_path(view: "month")
    expect(response).to have_http_status(:ok)
    expect(response.body).to include("Visible standup")
    expect(response.body).not_to include("Hidden retro")
  end

  it "the toggle is idempotent and reversible" do
    calendar = create_calendar(@account)

    2.times { patch calendar_visibility_path(calendar), params: { hidden: "1" } }
    expect(@user.reload.hidden_calendar_ids).to eq([ calendar.id.to_s ])

    patch calendar_visibility_path(calendar), params: { hidden: "0" }
    expect(@user.reload.hidden_calendar_ids).to be_empty
  end

  it "hiding is per-user, not account-wide" do
    calendar = create_calendar(@account)
    other = create_user("other")
    @account.calendar_account_users.create!(user: other, can_read: true)

    patch calendar_visibility_path(calendar), params: { hidden: "1" }

    expect(other.reload.hidden_calendar_ids).to be_empty
  end

  it "a calendar on an unshared account is not found" do
    stranger        = create_user("stranger")
    foreign_account = create_calendar_account(stranger)
    foreign_calendar = create_calendar(foreign_account)

    patch calendar_visibility_path(foreign_calendar), params: { hidden: "1" }

    expect(response).to have_http_status(:not_found)
  end

  # ── Index filtering ────────────────────────────────────────────────────────

  it "events from calendars that stopped syncing no longer render" do
    active = create_calendar(@account, name: "Active")
    paused = create_calendar(@account, name: "Paused", syncing: false)
    create_event(active, title: "Active planning")
    create_event(paused, title: "Stale kickoff")

    get calendar_path(view: "month")

    expect(response).to have_http_status(:ok)
    expect(response.body).to include("Active planning")
    expect(response.body).not_to include("Stale kickoff")
  end

  it "the sidebar lists the account with its synced and not-yet-synced calendars" do
    create_calendar(@account, name: "Team calendar")
    create_calendar(@account, name: "Holidays", syncing: false)

    get calendar_path

    expect(response).to have_http_status(:ok)
    expect(response.body).to include(@account.email_address)
    expect(response.body).to include("Team calendar")
    expect(response.body).to include("Holidays")
  end

  # ── Provider list refresh ──────────────────────────────────────────────────

  it "refresh enqueues a full scan for a manager" do
    expect {
      post refresh_calendar_account_path(@account)
    }.to have_enqueued_job(CalendarScanJob).with(@account.id, "full")

    expect(response).to have_http_status(:found)
  end

  it "refresh is denied for a viewer" do
    viewer = create_user("viewer")
    @account.calendar_account_users.create!(user: viewer, can_read: true)
    delete session_path  # drop the owner session from setup
    sign_in(viewer)

    expect {
      post refresh_calendar_account_path(@account)
    }.not_to have_enqueued_job(CalendarScanJob)

    expect(response).to redirect_to(calendar_path)
  end

  private

  def create_user(prefix)
    @workspace.users.create!(
      name: prefix.capitalize,
      email_address: "#{prefix}-#{SecureRandom.hex(4)}@example.com",
      password: "password123"
    )
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
