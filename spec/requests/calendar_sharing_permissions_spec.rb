require "rails_helper"

# Locks down the shared-calendar permission model: the accessible_to read gate
# (404 for events you cannot see, never 403), write guards on edit/delete/RSVP/
# reschedule, account-management gates (sharing panel, rename, sync toggle, member
# add), and share-revocation cutting off access immediately.
RSpec.describe "CalendarSharingPermissions", type: :request do
  before { WebMock.disable_net_connect! }
  after  { WebMock.allow_net_connect! }

  let(:workspace)   { create(:workspace) }
  let(:owner_user)  { create(:user, workspace: workspace) }
  let(:cal_account) { create(:calendar_account, workspace: workspace) }
  let(:calendar)    { create(:calendar, calendar_account: cal_account, is_writable: true, syncing: true) }
  let(:event)       { create(:calendar_event, calendar: calendar) }

  before do
    create(:calendar_account_user, :owner, user: owner_user, calendar_account: cal_account)
  end

  # -- read gate ---------------------------------------------------------------

  # Viewer row (can_read: true) is the only requirement for set_event to find the
  # event; the edit page renders a read-only view for non-writable users.
  it "a viewer can open the event edit page" do
    viewer = create(:user, workspace: workspace)
    create(:calendar_account_user, :viewer, user: viewer, calendar_account: cal_account)
    sign_in_as viewer

    get edit_calendar_event_path(event)
    expect(response).to have_http_status(:ok)
  end

  # No CalendarAccountUser row means accessible_to returns none -> RecordNotFound -> 404.
  it "a workspace member with no share gets 404 for the event" do
    no_share = create(:user, workspace: workspace)
    sign_in_as no_share

    get edit_calendar_event_path(event)
    expect(response).to have_http_status(:not_found)
  end

  # User from a different workspace also has no CalendarAccountUser row -> 404.
  it "a user from a different workspace gets 404 for the event" do
    outsider = create(:user)
    sign_in_as outsider

    get edit_calendar_event_path(event)
    expect(response).to have_http_status(:not_found)
  end

  # -- write guards ------------------------------------------------------------

  # require_writable_event redirects viewers before update runs; title unchanged.
  it "a viewer cannot update the event" do
    viewer = create(:user, workspace: workspace)
    create(:calendar_account_user, :viewer, user: viewer, calendar_account: cal_account)
    original_title = event.title
    sign_in_as viewer

    patch calendar_event_path(event), params: { calendar_event: { title: "Hacked Title" } }
    expect(response).to have_http_status(:redirect)
    expect(event.reload.title).to eq(original_title)
  end

  # require_writable_event redirects before destroy; event remains confirmed and
  # outbound_pending stays false (no tombstone write happened).
  it "a viewer cannot destroy the event" do
    viewer = create(:user, workspace: workspace)
    create(:calendar_account_user, :viewer, user: viewer, calendar_account: cal_account)
    sign_in_as viewer

    delete calendar_event_path(event)
    expect(response).to have_http_status(:redirect)
    expect(event.reload.confirmed?).to be true
    expect(event.reload.outbound_pending).to be false
  end

  # require_writable_event fires before rsvp; rsvp_status stays nil (never set).
  it "a viewer cannot RSVP; rsvp_status stays nil" do
    viewer = create(:user, workspace: workspace)
    create(:calendar_account_user, :viewer, user: viewer, calendar_account: cal_account)
    sign_in_as viewer

    post rsvp_calendar_event_path(event), params: { rsvp_status: "accepted" }
    expect(response).to have_http_status(:redirect)
    expect(event.reload.rsvp_status).to be_nil
  end

  # reschedule does its own inline check and returns head :forbidden (not redirect).
  it "a viewer gets 403 on reschedule and start_at is unchanged" do
    viewer = create(:user, workspace: workspace)
    create(:calendar_account_user, :viewer, user: viewer, calendar_account: cal_account)
    original_start = event.start_at
    sign_in_as viewer

    patch reschedule_calendar_event_path(event),
          params: { start_at: 2.days.from_now.iso8601, end_at: (2.days.from_now + 1.hour).iso8601 }
    expect(response).to have_http_status(:forbidden)
    expect(event.reload.start_at).to eq(original_start)
  end

  # Editor has can_write: true so writable_by? passes; title is persisted.
  it "an editor can update the event" do
    editor = create(:user, workspace: workspace)
    create(:calendar_account_user, :editor, user: editor, calendar_account: cal_account)
    sign_in_as editor

    patch calendar_event_path(event), params: { calendar_event: { title: "Updated by Editor" } }
    expect(response).to have_http_status(:redirect)
    expect(event.reload.title).to eq("Updated by Editor")
  end

  # -- account-management gates ------------------------------------------------

  # sharing action is owner-only; editor has an entry (so set_calendar_account
  # succeeds) but owned_by? is false -> redirect.
  it "editor cannot open the sharing panel" do
    editor = create(:user, workspace: workspace)
    create(:calendar_account_user, :editor, user: editor, calendar_account: cal_account)

    sign_in_as editor
    get sharing_calendar_account_path(cal_account)
    expect(response).to have_http_status(:redirect)
  end

  # The sharing page renders for the owner, listing current members and the
  # addable workspace teammates. (Regression: the template renders the settings
  # sidebar, which needs current_section from the controller — without it this
  # page 500s and calendar sharing is unmanageable in the UI.)
  it "the owner can open the sharing panel and see members and addable teammates" do
    teammate = create(:user, name: "Addable Teammate", workspace: workspace)
    sign_in_as owner_user

    get sharing_calendar_account_path(cal_account)

    expect(response).to have_http_status(:ok)
    expect(response.body).to match(owner_user.name)
    expect(response.body).to match(teammate.name)
  end

  # PATCH with params[:calendar_account] requires managed_by? (manager+); editor
  # is rejected and the name is unchanged; manager succeeds.
  it "editor cannot rename the calendar account; manager can" do
    editor = create(:user, workspace: workspace)
    create(:calendar_account_user, :editor, user: editor, calendar_account: cal_account)
    manager = create(:user, workspace: workspace)
    create(:calendar_account_user, :manager, user: manager, calendar_account: cal_account)
    # Give the account a concrete name so the before/after comparison is unambiguous.
    cal_account.update_column(:name, "Before Rename")

    sign_in_as editor
    patch calendar_account_path(cal_account), params: { calendar_account: { name: "Editor Rename" } }
    expect(response).to have_http_status(:redirect)
    expect(cal_account.reload.name).to eq("Before Rename")

    sign_in_as manager
    patch calendar_account_path(cal_account), params: { calendar_account: { name: "Manager Rename" } }
    expect(response).to have_http_status(:redirect)
    expect(cal_account.reload.name).to eq("Manager Rename")
  end

  # CalendarsController#update requires managed_by?; editor is rejected; manager
  # toggles syncing off successfully.
  it "manager can toggle calendar syncing; editor cannot" do
    editor = create(:user, workspace: workspace)
    create(:calendar_account_user, :editor, user: editor, calendar_account: cal_account)
    manager = create(:user, workspace: workspace)
    create(:calendar_account_user, :manager, user: manager, calendar_account: cal_account)

    sign_in_as editor
    patch calendar_account_calendar_path(cal_account, calendar), params: { calendar: { syncing: false } }
    expect(response).to have_http_status(:redirect)
    expect(calendar.reload.syncing).to be true

    sign_in_as manager
    patch calendar_account_calendar_path(cal_account, calendar), params: { calendar: { syncing: false } }
    expect(response).to have_http_status(:redirect)
    expect(calendar.reload.syncing).to be false
  end

  # PATCH without params[:calendar_account] goes to update_user_permissions which
  # requires owned_by?; manager is denied and member count is unchanged; owner
  # can add the member successfully.
  it "only the owner can add a sharing member; manager is denied" do
    manager = create(:user, workspace: workspace)
    create(:calendar_account_user, :manager, user: manager, calendar_account: cal_account)
    new_member = create(:user, workspace: workspace)

    sign_in_as manager
    initial_count = cal_account.calendar_account_users.count
    patch calendar_account_path(cal_account), params: { user_email: new_member.email_address, role: "viewer" }
    expect(response).to have_http_status(:redirect)
    expect(cal_account.calendar_account_users.count).to eq(initial_count)

    sign_in_as owner_user
    patch calendar_account_path(cal_account), params: { user_email: new_member.email_address, role: "viewer" }
    expect(response).to have_http_status(:redirect)
    expect(cal_account.calendar_account_users.exists?(user: new_member)).to be true
  end

  # -- revocation --------------------------------------------------------------

  # Destroying the CalendarAccountUser row immediately removes the user from
  # readable_calendar_accounts; the next request returns 404.
  it "revoking the viewer share cuts off access on the next request" do
    viewer = create(:user, workspace: workspace)
    entry = create(:calendar_account_user, :viewer, user: viewer, calendar_account: cal_account)
    sign_in_as viewer

    get edit_calendar_event_path(event)
    expect(response).to have_http_status(:ok)

    entry.destroy!

    get edit_calendar_event_path(event)
    expect(response).to have_http_status(:not_found)
  end

  # -- create gate -------------------------------------------------------------

  # writable_calendars scopes to writable_calendar_accounts (can_write: true);
  # a viewer cannot POST a new event (calendar not found -> redirect), while an
  # editor successfully saves one.
  it "an editor can create an event; a viewer is redirected with no record created" do
    editor = create(:user, workspace: workspace)
    create(:calendar_account_user, :editor, user: editor, calendar_account: cal_account)
    viewer = create(:user, workspace: workspace)
    create(:calendar_account_user, :viewer, user: viewer, calendar_account: cal_account)

    event_params = {
      calendar_event: {
        title: "New Meeting",
        calendar_id: calendar.id,
        start_at: 3.days.from_now.beginning_of_hour.iso8601,
        end_at:   (3.days.from_now.beginning_of_hour + 1.hour).iso8601
      }
    }

    sign_in_as viewer
    expect {
      post calendar_events_path, params: event_params
    }.not_to change(CalendarEvent, :count)
    expect(response).to have_http_status(:redirect)

    sign_in_as editor
    expect {
      post calendar_events_path, params: event_params
    }.to change(CalendarEvent, :count).by(1)
  end
end
