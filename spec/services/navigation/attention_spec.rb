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
      create(:document, :in_review, workspace: user.workspace)
      expect(attention.dot?(:documents)).to be true

      user.workspace.documents.needs_review.update_all(review_status: :approved)
      expect(attention.dot?(:documents)).to be true

      expect(described_class.new(user).dot?(:documents)).to be false
    end
  end

  describe ":mail" do
    let(:account) { create(:email_account, workspace: user.workspace) }

    before { create(:email_account_user, user: user, email_account: account, can_read: true) }

    it "lights up for unskimmed mail" do
      create(:email_message, email_account: account, skimmed_at: nil)
      expect(attention.dot?(:mail)).to be true
    end

    it "stays clear when all mail is skimmed" do
      create(:email_message, email_account: account, skimmed_at: Time.current)
      expect(attention.dot?(:mail)).to be false
    end

    it "ignores mail on accounts the user cannot read" do
      other = create(:email_account, workspace: user.workspace)
      create(:email_message, email_account: other, skimmed_at: nil)
      expect(attention.dot?(:mail)).to be false
    end
  end

  describe ":calendar" do
    it "lights up for a pending reminder" do
      create(:reminder, workspace: user.workspace)
      expect(attention.dot?(:calendar)).to be true
    end

    it "stays clear when reminders are confirmed" do
      create(:reminder, :confirmed, workspace: user.workspace)
      expect(attention.dot?(:calendar)).to be false
    end

    it "stays clear when reminders are dismissed" do
      create(:reminder, status: :dismissed, workspace: user.workspace)
      expect(attention.dot?(:calendar)).to be false
    end

    it "ignores calendar events (events are a view, not action items)" do
      account = create(:calendar_account, workspace: user.workspace)
      create(:calendar_account_user, user: user, calendar_account: account, can_read: true)
      calendar = create(:calendar, calendar_account: account)
      create(:calendar_event, calendar: calendar)
      expect(attention.dot?(:calendar)).to be false
    end
  end

  describe ":documents" do
    it "lights up for a document needing review" do
      create(:document, :in_review, workspace: user.workspace)
      expect(attention.dot?(:documents)).to be true
    end

    it "stays clear when review is approved" do
      create(:document, :approved, workspace: user.workspace)
      expect(attention.dot?(:documents)).to be false
    end

    it "ignores documents in another workspace" do
      create(:document, :in_review)
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

    it "lights up for an active feed item" do
      feed_item
      expect(attention.dot?(:home)).to be true
    end

    it "ignores dismissed or acted items" do
      feed_item(dismissed_at: Time.current)
      feed_item(acted_at: Time.current)
      expect(attention.dot?(:home)).to be false
    end
  end
end
