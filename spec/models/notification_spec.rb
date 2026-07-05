require "rails_helper"

RSpec.describe Notification, type: :model do
  # Turbo broadcasts render partials over ActionCable; stub them so model
  # behaviour can be tested in isolation.
  before do
    allow_any_instance_of(Notification).to receive(:broadcast_replace_to)
    allow_any_instance_of(Notification).to receive(:broadcast_remove_to)
    allow_any_instance_of(Notification).to receive(:broadcast_append_to)
  end

  let(:user) { create(:user) }

  describe ".notify" do
    it "creates a notification with category, priority and notifiable" do
      account = create(:email_account, workspace: user.workspace)
      n = Notification.notify(user: user, category: :system, priority: :action_required,
            title: "x", group_key: "k/1", notifiable: account)

      expect(n).to be_persisted
      expect(n).to be_category_system
      expect(n).to be_priority_action_required
      expect(n.notifiable).to eq(account)
    end

    it "groups repeat events on the same key into one active notification" do
      2.times { Notification.notify(user: user, category: :activity, priority: :activity, title: "x", group_key: "g/1") }

      expect(user.notifications.where(group_key: "g/1").count).to eq(1)
      expect(user.notifications.find_by(group_key: "g/1").count).to eq(2)
    end

    it "suppresses lower-tier notifications when the user opted out" do
      tag = Tag.create!(workspace: user.workspace, name: "Invoices", color: "#ccc")
      user.notification_preferences.create!(kind: :tag, tag_id: tag.id, notify_in_app: false)

      n = Notification.notify(user: user, category: :activity, priority: :activity, title: "x",
            preference: { kind: :tag, id: tag.id })

      expect(n).to be_nil
    end

    it "always fires action_required regardless of preferences" do
      tag = Tag.create!(workspace: user.workspace, name: "Invoices", color: "#ccc")
      user.notification_preferences.create!(kind: :tag, tag_id: tag.id, notify_in_app: false)

      n = Notification.notify(user: user, category: :system, priority: :action_required, title: "x",
            preference: { kind: :tag, id: tag.id })

      expect(n).to be_persisted
    end
  end

  describe ".resolve" do
    it "resolves active notifications for the subject and drops them from active/badge" do
      account = create(:email_account, workspace: user.workspace)
      Notification.notify(user: user, category: :system, priority: :action_required,
        title: "x", notifiable: account, group_key: "k/#{account.id}")
      expect(user.notifications.badge_visible.count).to eq(1)

      Notification.resolve(notifiable: account, category: :system)

      expect(user.notifications.active.count).to eq(0)
      expect(user.notifications.badge_visible.count).to eq(0)
    end
  end

  describe "#archive! / #unarchive!" do
    it "archives then restores" do
      n = create(:notification, user: user)

      n.archive!
      expect(n).to be_archived
      expect(user.notifications.active).to be_empty

      n.unarchive!
      expect(n.reload).not_to be_archived
      expect(user.notifications.active).to contain_exactly(n)
    end
  end

  describe "scopes" do
    it "badge_visible counts only unread, unarchived, unresolved" do
      create(:notification, user: user, read: false)
      create(:notification, user: user, read: true)
      create(:notification, user: user, read: false, archived_at: Time.current)
      create(:notification, user: user, read: false, resolved_at: Time.current)

      expect(user.notifications.badge_visible.count).to eq(1)
      expect(user.unread_notifications_count).to eq(1)
    end

    it "needs_action returns active action_required notifications" do
      wanted = create(:notification, user: user, priority: :action_required)
      create(:notification, user: user, priority: :awaiting)

      expect(user.notifications.needs_action).to contain_exactly(wanted)
    end
  end

  describe "#toast_suppressed?" do
    it "suppresses activity and ai_reply, but not other awaiting categories" do
      expect(build(:notification, priority: :activity).send(:toast_suppressed?)).to be(true)
      expect(build(:notification, category: :ai_reply, priority: :awaiting).send(:toast_suppressed?)).to be(true)
      expect(build(:notification, category: :export, priority: :awaiting).send(:toast_suppressed?)).to be(false)
    end
  end

  # ── From NotificationTest (Minitest migration) ───────────────────────────────

  describe ".notify (extended coverage)" do
    before do
      @workspace = create(:workspace)
      @user      = create(:user, workspace: @workspace)
    end

    it "creates a notification with correct attributes" do
      notifiable_thread = create(:agent_thread, user: @user, workspace: @workspace)

      notif = Notification.notify(
        user:      @user,
        category:  :mention,
        priority:  :awaiting,
        title:     "Someone mentioned you",
        body:      "Check this out",
        link_url:  "/email_threads/42",
        notifiable: notifiable_thread
      )

      expect(notif).not_to be_nil
      expect(notif.user).to eq(@user)
      expect(notif).to be_category_mention
      expect(notif).to be_priority_awaiting
      expect(notif.title).to eq("Someone mentioned you")
      expect(notif.link_url).to eq("/email_threads/42")
      expect(notif.notifiable).to eq(notifiable_thread)
      expect(notif.count).to eq(1)
    end

    it "with same group_key within window bumps count and resets read" do
      key = "thread_activity/99"

      first = Notification.notify(
        user: @user, category: :comment, priority: :activity, title: "New activity", group_key: key
      )
      first.update!(read: true) # simulate user having read it

      expect {
        second = Notification.notify(
          user: @user, category: :comment, priority: :activity, title: "Another activity", group_key: key
        )
        expect(second.id).to eq(first.id), "should return the same row"
        expect(second.reload.count).to eq(2), "count should be bumped to 2"
        expect(second.read).to eq(false), "read should be reset to false"
      }.not_to change(Notification, :count)
    end

    it "with same group_key outside window creates a new row" do
      key = "thread_activity/old"

      old_notif = Notification.notify(
        user: @user, category: :comment, priority: :activity, title: "Old activity", group_key: key
      )
      old_notif.update_column(:created_at, 6.minutes.ago)

      expect {
        Notification.notify(
          user: @user, category: :comment, priority: :activity, title: "Fresh activity", group_key: key
        )
      }.to change(Notification, :count).by(1)

      rows = @user.notifications.where(group_key: key).order(:created_at)
      expect(rows.count).to eq(2), "should have two separate notification rows after the window expires"
    end

    it "notify_in_app:false suppresses non-action-required but not action_required" do
      tag  = Tag.create!(workspace: @workspace, name: "Finance", color: "#6366f1")
      @user.notification_preferences.create!(kind: :tag, tag_id: tag.id, notify_in_app: false)
      preference = { kind: :tag, id: tag.id }

      # Activity tier: suppressed.
      result = Notification.notify(
        user: @user, category: :activity, priority: :activity, title: "Tag activity", preference: preference
      )
      expect(result).to be_nil, "activity notification should be suppressed by preference"

      # Awaiting tier: also suppressed.
      result = Notification.notify(
        user: @user, category: :document, priority: :awaiting, title: "Document ready", preference: preference
      )
      expect(result).to be_nil, "awaiting notification should be suppressed by preference"

      # Action-required: bypasses opt-out.
      expect {
        Notification.notify(
          user: @user, category: :document, priority: :action_required,
          title: "Document needs review", preference: preference
        )
      }.to change(Notification, :count).by(1)
    end
  end

  describe ".resolve (extended coverage)" do
    before do
      @workspace = create(:workspace)
      @user      = create(:user, workspace: @workspace)
    end

    it "resolves all matching active notifications" do
      agent_thread = create(:agent_thread, user: @user, workspace: @workspace)

      n1 = create(:notification, user: @user, category: :mention,  notifiable: agent_thread)
      n2 = create(:notification, user: @user, category: :mention,  notifiable: agent_thread)
      n3 = create(:notification, user: @user, category: :activity, notifiable: agent_thread)

      Notification.resolve(notifiable: agent_thread, category: :mention)

      expect(n1.reload).to be_resolved, "n1 (mention) should be resolved"
      expect(n2.reload).to be_resolved, "n2 (mention) should be resolved"
      expect(n3.reload).not_to be_resolved, "n3 (activity) should not be resolved — different category"
    end
  end

  describe "badge_visible (extended coverage)" do
    before do
      @workspace = create(:workspace)
      @user      = create(:user, workspace: @workspace)
    end

    it "counts only unread active notifications" do
      unread   = create(:notification, user: @user, read: false)
      read_one = create(:notification, user: @user, read: true)
      archived = create(:notification, user: @user, read: false).tap(&:archive!)
      resolved = create(:notification, user: @user, read: false).tap(&:resolve!)

      visible = @user.notifications.badge_visible.to_a
      expect(visible).to include(unread),      "unread active should be badge visible"
      expect(visible).not_to include(read_one), "read notification should not be badge visible"
      expect(visible).not_to include(archived), "archived notification should not be badge visible"
      expect(visible).not_to include(resolved), "resolved notification should not be badge visible"
    end
  end

  describe "#toast_suppressed? (extended coverage)" do
    before do
      @workspace = create(:workspace)
      @user      = create(:user, workspace: @workspace)
    end

    it "is true only for activity and ai_reply" do
      activity_notif   = build(:notification, user: @user, priority: :activity, category: :comment)
      expect(activity_notif.send(:toast_suppressed?)).to be_truthy,
        "priority :activity should suppress toast"

      ai_reply_notif   = build(:notification, user: @user, priority: :awaiting, category: :ai_reply)
      expect(ai_reply_notif.send(:toast_suppressed?)).to be_truthy,
        "category :ai_reply should suppress toast"

      awaiting_mention = build(:notification, user: @user, priority: :awaiting, category: :mention)
      expect(awaiting_mention.send(:toast_suppressed?)).to be_falsey,
        "awaiting mention should NOT suppress toast"

      action_req = build(:notification, user: @user, priority: :action_required, category: :system)
      expect(action_req.send(:toast_suppressed?)).to be_falsey,
        "action_required should NOT suppress toast"
    end
  end

  describe "native push delivery (after_create_commit)" do
    it "enqueues PushDeliveryJob for tiers that warrant a toast" do
      expect {
        create(:notification, user: user, category: :document, priority: :awaiting)
      }.to have_enqueued_job(PushDeliveryJob)
    end

    it "does not push quiet tiers (activity / ai_reply)" do
      expect {
        create(:notification, user: user, priority: :activity)
      }.not_to have_enqueued_job(PushDeliveryJob)

      expect {
        create(:notification, user: user, category: :ai_reply, priority: :awaiting)
      }.not_to have_enqueued_job(PushDeliveryJob)
    end
  end
end
