require "test_helper"

# Unit coverage for Notification.notify grouping, preference suppression,
# resolve, badge scopes, and toast suppression invariants.
class NotificationTest < ActiveSupport::TestCase
  setup do
    @workspace = create(:workspace)
    @user = create(:user, workspace: @workspace)
  end

  # 1. .notify creates a notification with the supplied category, priority,
  #    link_url, and notifiable.
  test ".notify creates a notification with correct attributes" do
    notifiable_thread = create(:agent_thread, user: @user, workspace: @workspace)

    notif = Notification.notify(
      user: @user,
      category: :mention,
      priority: :awaiting,
      title: "Someone mentioned you",
      body: "Check this out",
      link_url: "/email_threads/42",
      notifiable: notifiable_thread
    )

    assert_not_nil notif
    assert_equal @user,             notif.user
    assert notif.category_mention?,  "expected category :mention"
    assert notif.priority_awaiting?, "expected priority :awaiting"
    assert_equal "Someone mentioned you", notif.title
    assert_equal "/email_threads/42",     notif.link_url
    assert_equal notifiable_thread,       notif.notifiable
    assert_equal 1,                       notif.count
  end

  # 2. Calling .notify with the same group_key within the group window bumps
  #    count on the existing row (no second row) and resets read to false.
  test ".notify with same group_key within window bumps count and resets read" do
    key = "thread_activity/99"

    first = Notification.notify(
      user: @user,
      category: :comment,
      priority: :activity,
      title: "New activity",
      group_key: key
    )
    first.update!(read: true)  # simulate user having read it

    assert_no_difference "Notification.count" do
      second = Notification.notify(
        user: @user,
        category: :comment,
        priority: :activity,
        title: "Another activity",
        group_key: key
      )
      assert_equal first.id, second.id, "should return the same row"
      assert_equal 2, second.reload.count, "count should be bumped to 2"
      assert_equal false, second.read,     "read should be reset to false"
    end
  end

  # 3. The same group_key OUTSIDE the window creates a new row.
  #    group_window defaults to 5 minutes; place the first notification in the
  #    past so the second call falls outside the window.
  test ".notify with same group_key outside window creates a new row" do
    key = "thread_activity/old"

    # Create a notification and manually backdate it past the 5-minute window.
    old_notif = Notification.notify(
      user: @user,
      category: :comment,
      priority: :activity,
      title: "Old activity",
      group_key: key
    )
    old_notif.update_column(:created_at, 6.minutes.ago)

    assert_difference "Notification.count", 1 do
      Notification.notify(
        user: @user,
        category: :comment,
        priority: :activity,
        title: "Fresh activity",
        group_key: key
      )
    end

    rows = @user.notifications.where(group_key: key).order(:created_at)
    assert_equal 2, rows.count,
                 "should have two separate notification rows after the window expires"
  end

  # 4. A NotificationPreference with notify_in_app: false suppresses an activity
  #    (or awaiting) notification for that preference target, but does NOT
  #    suppress an action_required one.
  test "notify_in_app:false suppresses non-action-required but not action_required" do
    tag = Tag.create!(workspace: @workspace, name: "Finance", color: "#6366f1")

    pref = @user.notification_preferences.create!(
      kind: :tag,
      tag_id: tag.id,
      notify_in_app: false
    )
    preference = { kind: :tag, id: tag.id }

    # Activity tier: suppressed.
    result = Notification.notify(
      user: @user,
      category: :activity,
      priority: :activity,
      title: "Tag activity",
      preference: preference
    )
    assert_nil result, "activity notification should be suppressed by preference"

    # Awaiting tier: also suppressed.
    result = Notification.notify(
      user: @user,
      category: :document,
      priority: :awaiting,
      title: "Document ready",
      preference: preference
    )
    assert_nil result, "awaiting notification should be suppressed by preference"

    # Action-required: bypasses opt-out (respect_preferences defaults true but
    # action_required is always raised per Notification.notify logic).
    assert_difference "Notification.count", 1 do
      Notification.notify(
        user: @user,
        category: :document,
        priority: :action_required,
        title: "Document needs review",
        preference: preference
      )
    end
  end

  # 5. .resolve(notifiable:, category:) resolves every active notification
  #    that matches both the notifiable and the category.
  test ".resolve resolves all matching active notifications" do
    agent_thread = create(:agent_thread, user: @user, workspace: @workspace)

    n1 = create(:notification, user: @user, category: :mention,  notifiable: agent_thread)
    n2 = create(:notification, user: @user, category: :mention,  notifiable: agent_thread)
    n3 = create(:notification, user: @user, category: :activity, notifiable: agent_thread)

    Notification.resolve(notifiable: agent_thread, category: :mention)

    assert n1.reload.resolved?, "n1 (mention) should be resolved"
    assert n2.reload.resolved?, "n2 (mention) should be resolved"
    assert_not n3.reload.resolved?, "n3 (activity) should not be resolved — different category"
  end

  # 6. badge_visible counts only unread + active (not archived, resolved, or read).
  test "badge_visible counts only unread active notifications" do
    unread   = create(:notification, user: @user, read: false)
    read_one = create(:notification, user: @user, read: true)
    archived = create(:notification, user: @user, read: false).tap(&:archive!)
    resolved = create(:notification, user: @user, read: false).tap(&:resolve!)

    visible = @user.notifications.badge_visible.to_a
    assert_includes visible, unread,   "unread active should be badge visible"
    assert_not_includes visible, read_one, "read notification should not be badge visible"
    assert_not_includes visible, archived, "archived notification should not be badge visible"
    assert_not_includes visible, resolved, "resolved notification should not be badge visible"
  end

  # 7. toast_suppressed? is true for priority :activity and category :ai_reply,
  #    false for action_required and awaiting/mention.
  test "toast_suppressed? is true only for activity and ai_reply" do
    activity_notif = build(:notification, user: @user, priority: :activity, category: :comment)
    assert activity_notif.send(:toast_suppressed?),
           "priority :activity should suppress toast"

    ai_reply_notif = build(:notification, user: @user, priority: :awaiting, category: :ai_reply)
    assert ai_reply_notif.send(:toast_suppressed?),
           "category :ai_reply should suppress toast"

    awaiting_mention = build(:notification, user: @user, priority: :awaiting, category: :mention)
    assert_not awaiting_mention.send(:toast_suppressed?),
               "awaiting mention should NOT suppress toast"

    action_req = build(:notification, user: @user, priority: :action_required, category: :system)
    assert_not action_req.send(:toast_suppressed?),
               "action_required should NOT suppress toast"
  end
end
