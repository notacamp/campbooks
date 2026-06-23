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
