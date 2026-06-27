require "rails_helper"

RSpec.describe Navigation::Attention do
  let(:user) { create(:user) }
  subject(:attention) { described_class.new(user) }

  describe "#dot?" do
    it "fails closed for a nil user" do
      expect(described_class.new(nil).dot?(:mail)).to be false
    end

    it "is false for an unknown section" do
      expect(attention.dot?(:bogus)).to be false
    end

    it "memoizes within the instance" do
      create(:document, :in_review, workspace: user.workspace, viewed_at: nil)
      expect(attention.dot?(:documents)).to be true

      user.workspace.documents.needs_review.where(viewed_at: nil).update_all(viewed_at: Time.current)
      expect(attention.dot?(:documents)).to be true # memoized

      expect(described_class.new(user).dot?(:documents)).to be false
    end
  end

  describe ":mail" do
    let(:account) { create(:email_account, workspace: user.workspace) }

    before { create(:email_account_user, user: user, email_account: account, can_read: true) }

    it "lights up for unread mail" do
      create(:email_message, email_account: account, read: false)
      expect(attention.dot?(:mail)).to be true
    end

    it "stays clear when mail is read" do
      create(:email_message, email_account: account, read: true)
      expect(attention.dot?(:mail)).to be false
    end

    it "ignores mail on accounts the user cannot read" do
      other = create(:email_account, workspace: user.workspace)
      create(:email_message, email_account: other, read: false)
      expect(attention.dot?(:mail)).to be false
    end
  end

  describe ":calendar" do
    it "lights up for an unviewed pending reminder" do
      create(:reminder, workspace: user.workspace, viewed_at: nil)
      expect(attention.dot?(:calendar)).to be true
    end

    it "stays clear when the reminder has been viewed" do
      create(:reminder, workspace: user.workspace, viewed_at: Time.current)
      expect(attention.dot?(:calendar)).to be false
    end

    it "stays clear when reminders are confirmed (even if unviewed)" do
      create(:reminder, :confirmed, workspace: user.workspace, viewed_at: nil)
      expect(attention.dot?(:calendar)).to be false
    end

    it "stays clear when reminders are dismissed (even if unviewed)" do
      create(:reminder, status: :dismissed, workspace: user.workspace, viewed_at: nil)
      expect(attention.dot?(:calendar)).to be false
    end

    it "ignores calendar events" do
      account = create(:calendar_account, workspace: user.workspace)
      create(:calendar_account_user, user: user, calendar_account: account, can_read: true)
      calendar = create(:calendar, calendar_account: account)
      create(:calendar_event, calendar: calendar)
      expect(attention.dot?(:calendar)).to be false
    end
  end

  describe ":documents" do
    it "lights up for an unviewed document needing review" do
      create(:document, :in_review, workspace: user.workspace, viewed_at: nil)
      expect(attention.dot?(:documents)).to be true
    end

    it "stays clear when the document has been viewed" do
      create(:document, :in_review, workspace: user.workspace, viewed_at: Time.current)
      expect(attention.dot?(:documents)).to be false
    end

    it "stays clear when review is approved (even if unviewed)" do
      create(:document, :approved, workspace: user.workspace, viewed_at: nil)
      expect(attention.dot?(:documents)).to be false
    end

    it "ignores documents in another workspace" do
      create(:document, :in_review, viewed_at: nil)
      expect(attention.dot?(:documents)).to be false
    end
  end

  describe ":scout" do
    let(:thread) { create(:agent_thread, user: user) }

    it "lights up for an unread AI reply" do
      create(:agent_message, agent_thread: thread, author_type: :ai, read: false)
      expect(attention.dot?(:scout)).to be true
    end

    it "stays clear when AI messages are read" do
      create(:agent_message, agent_thread: thread, author_type: :ai, read: true)
      expect(attention.dot?(:scout)).to be false
    end

    it "ignores the user's own messages" do
      create(:agent_message, agent_thread: thread, author_type: :user, read: false)
      expect(attention.dot?(:scout)).to be false
    end

    it "ignores AI messages in setup_chat threads" do
      setup = create(:agent_thread, user: user, purpose: :setup_chat)
      create(:agent_message, agent_thread: setup, author_type: :ai, read: false)
      expect(attention.dot?(:scout)).to be false
    end
  end

  describe ":home" do
    def feed_item(**overrides)
      FeedItem.create!({
        user: user, workspace: user.workspace, kind: "reminder",
        subject: create(:document, workspace: user.workspace),
        dedupe_key: "k:#{SecureRandom.hex(4)}", sort_at: Time.current, generated_at: Time.current
      }.merge(overrides))
    end

    it "lights up for an unseen, active feed item" do
      feed_item(seen_at: nil)
      expect(attention.dot?(:home)).to be true
    end

    it "stays clear when the feed item has been seen" do
      feed_item(seen_at: Time.current)
      expect(attention.dot?(:home)).to be false
    end

    it "ignores dismissed items (even if unseen)" do
      feed_item(seen_at: nil, dismissed_at: Time.current)
      expect(attention.dot?(:home)).to be false
    end

    it "ignores acted items (even if unseen)" do
      feed_item(seen_at: nil, acted_at: Time.current)
      expect(attention.dot?(:home)).to be false
    end
  end
end
