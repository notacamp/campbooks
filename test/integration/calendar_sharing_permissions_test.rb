require "test_helper"

# Locks down the shared-calendar permission model: the accessible_to read gate
# (404 for events you cannot see, never 403), write guards on edit/delete/RSVP/
# reschedule, account-management gates (sharing panel, rename, sync toggle, member
# add), and share-revocation cutting off access immediately.
class CalendarSharingPermissionsTest < ActionDispatch::IntegrationTest
  setup do
    WebMock.disable_net_connect!

    @workspace    = create(:workspace)
    @owner_user   = create(:user, workspace: @workspace)
    @cal_account  = create(:calendar_account, workspace: @workspace)
    create(:calendar_account_user, :owner, user: @owner_user, calendar_account: @cal_account)
    @calendar = create(:calendar, calendar_account: @cal_account, is_writable: true, syncing: true)
    @event    = create(:calendar_event, calendar: @calendar)
  end

  teardown do
    WebMock.allow_net_connect!
  end

  # ── read gate ────────────────────────────────────────────────────────────

  # Viewer row (can_read: true) is the only requirement for set_event to find the
  # event; the edit page renders a read-only view for non-writable users.
  test "a viewer can open the event edit page" do
    viewer = create(:user, workspace: @workspace)
    create(:calendar_account_user, :viewer, user: viewer, calendar_account: @cal_account)
    sign_in_as viewer

    get edit_calendar_event_path(@event)
    assert_response :success
  end

  # No CalendarAccountUser row means accessible_to returns none -> RecordNotFound -> 404.
  test "a workspace member with no share gets 404 for the event" do
    no_share = create(:user, workspace: @workspace)
    sign_in_as no_share

    get edit_calendar_event_path(@event)
    assert_response :not_found
  end

  # User from a different workspace also has no CalendarAccountUser row -> 404.
  test "a user from a different workspace gets 404 for the event" do
    outsider = create(:user)
    sign_in_as outsider

    get edit_calendar_event_path(@event)
    assert_response :not_found
  end

  # ── write guards ─────────────────────────────────────────────────────────

  # require_writable_event redirects viewers before update runs; title unchanged.
  test "a viewer cannot update the event" do
    viewer = create(:user, workspace: @workspace)
    create(:calendar_account_user, :viewer, user: viewer, calendar_account: @cal_account)
    original_title = @event.title
    sign_in_as viewer

    patch calendar_event_path(@event), params: { calendar_event: { title: "Hacked Title" } }
    assert_response :redirect
    assert_equal original_title, @event.reload.title
  end

  # require_writable_event redirects before destroy; event remains confirmed and
  # outbound_pending stays false (no tombstone write happened).
  test "a viewer cannot destroy the event" do
    viewer = create(:user, workspace: @workspace)
    create(:calendar_account_user, :viewer, user: viewer, calendar_account: @cal_account)
    sign_in_as viewer

    delete calendar_event_path(@event)
    assert_response :redirect
    assert @event.reload.confirmed?
    refute @event.reload.outbound_pending
  end

  # require_writable_event fires before rsvp; rsvp_status stays nil (never set).
  test "a viewer cannot RSVP; rsvp_status stays nil" do
    viewer = create(:user, workspace: @workspace)
    create(:calendar_account_user, :viewer, user: viewer, calendar_account: @cal_account)
    sign_in_as viewer

    post rsvp_calendar_event_path(@event), params: { rsvp_status: "accepted" }
    assert_response :redirect
    assert_nil @event.reload.rsvp_status
  end

  # reschedule does its own inline check and returns head :forbidden (not redirect).
  test "a viewer gets 403 on reschedule and start_at is unchanged" do
    viewer = create(:user, workspace: @workspace)
    create(:calendar_account_user, :viewer, user: viewer, calendar_account: @cal_account)
    original_start = @event.start_at
    sign_in_as viewer

    patch reschedule_calendar_event_path(@event),
          params: { start_at: 2.days.from_now.iso8601, end_at: (2.days.from_now + 1.hour).iso8601 }
    assert_response :forbidden
    assert_equal original_start, @event.reload.start_at
  end

  # Editor has can_write: true so writable_by? passes; title is persisted.
  test "an editor can update the event" do
    editor = create(:user, workspace: @workspace)
    create(:calendar_account_user, :editor, user: editor, calendar_account: @cal_account)
    sign_in_as editor

    patch calendar_event_path(@event), params: { calendar_event: { title: "Updated by Editor" } }
    assert_response :redirect
    assert_equal "Updated by Editor", @event.reload.title
  end

  # ── account-management gates ─────────────────────────────────────────────

  # sharing action is owner-only; editor has an entry (so set_calendar_account
  # succeeds) but owned_by? is false -> redirect.
  test "editor cannot open the sharing panel; owner can" do
    editor = create(:user, workspace: @workspace)
    create(:calendar_account_user, :editor, user: editor, calendar_account: @cal_account)

    sign_in_as editor
    get sharing_calendar_account_path(@cal_account)
    assert_response :redirect

    sign_in_as @owner_user
    get sharing_calendar_account_path(@cal_account)
    assert_response :success
  end

  # PATCH with params[:calendar_account] requires managed_by? (manager+); editor
  # is rejected and the name is unchanged; manager succeeds.
  test "editor cannot rename the calendar account; manager can" do
    editor = create(:user, workspace: @workspace)
    create(:calendar_account_user, :editor, user: editor, calendar_account: @cal_account)
    manager = create(:user, workspace: @workspace)
    create(:calendar_account_user, :manager, user: manager, calendar_account: @cal_account)
    original_name = @cal_account.name

    sign_in_as editor
    patch calendar_account_path(@cal_account), params: { calendar_account: { name: "Editor Rename" } }
    assert_response :redirect
    assert_equal original_name, @cal_account.reload.name

    sign_in_as manager
    patch calendar_account_path(@cal_account), params: { calendar_account: { name: "Manager Rename" } }
    assert_response :redirect
    assert_equal "Manager Rename", @cal_account.reload.name
  end

  # CalendarsController#update requires managed_by?; editor is rejected; manager
  # toggles syncing off successfully.
  test "manager can toggle calendar syncing; editor cannot" do
    editor = create(:user, workspace: @workspace)
    create(:calendar_account_user, :editor, user: editor, calendar_account: @cal_account)
    manager = create(:user, workspace: @workspace)
    create(:calendar_account_user, :manager, user: manager, calendar_account: @cal_account)

    sign_in_as editor
    patch calendar_account_calendar_path(@cal_account, @calendar), params: { calendar: { syncing: false } }
    assert_response :redirect
    assert @calendar.reload.syncing, "editor must not be able to toggle syncing"

    sign_in_as manager
    patch calendar_account_calendar_path(@cal_account, @calendar), params: { calendar: { syncing: false } }
    assert_response :redirect
    refute @calendar.reload.syncing, "manager must be able to toggle syncing off"
  end

  # PATCH without params[:calendar_account] goes to update_user_permissions which
  # requires owned_by?; manager is denied and member count is unchanged; owner
  # can add the member successfully.
  test "only the owner can add a sharing member; manager is denied" do
    manager = create(:user, workspace: @workspace)
    create(:calendar_account_user, :manager, user: manager, calendar_account: @cal_account)
    new_member = create(:user, workspace: @workspace)

    sign_in_as manager
    initial_count = @cal_account.calendar_account_users.count
    patch calendar_account_path(@cal_account), params: { user_email: new_member.email_address, role: "viewer" }
    assert_response :redirect
    assert_equal initial_count, @cal_account.calendar_account_users.count

    sign_in_as @owner_user
    patch calendar_account_path(@cal_account), params: { user_email: new_member.email_address, role: "viewer" }
    assert_response :redirect
    assert @cal_account.calendar_account_users.exists?(user: new_member),
           "owner must be able to add a member via the sharing form"
  end

  # ── revocation ───────────────────────────────────────────────────────────

  # Destroying the CalendarAccountUser row immediately removes the user from
  # readable_calendar_accounts; the next request returns 404.
  test "revoking the viewer share cuts off access on the next request" do
    viewer = create(:user, workspace: @workspace)
    entry = create(:calendar_account_user, :viewer, user: viewer, calendar_account: @cal_account)
    sign_in_as viewer

    get edit_calendar_event_path(@event)
    assert_response :success

    entry.destroy!

    get edit_calendar_event_path(@event)
    assert_response :not_found
  end

  # ── create gate ──────────────────────────────────────────────────────────

  # writable_calendars scopes to writable_calendar_accounts (can_write: true);
  # a viewer cannot POST a new event (calendar not found -> redirect), while an
  # editor successfully saves one.
  test "an editor can create an event; a viewer is redirected with no record created" do
    editor = create(:user, workspace: @workspace)
    create(:calendar_account_user, :editor, user: editor, calendar_account: @cal_account)
    viewer = create(:user, workspace: @workspace)
    create(:calendar_account_user, :viewer, user: viewer, calendar_account: @cal_account)

    event_params = {
      calendar_event: {
        title: "New Meeting",
        calendar_id: @calendar.id,
        start_at: 3.days.from_now.beginning_of_hour.iso8601,
        end_at:   (3.days.from_now.beginning_of_hour + 1.hour).iso8601
      }
    }

    sign_in_as viewer
    assert_no_difference "CalendarEvent.count" do
      post calendar_events_path, params: event_params
    end
    assert_response :redirect

    sign_in_as editor
    assert_difference "CalendarEvent.count", 1 do
      post calendar_events_path, params: event_params
    end
  end
end
