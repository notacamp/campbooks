require "rails_helper"

RSpec.describe Navigation::Attention do
  let(:user) { create(:user) }
  subject(:attention) { described_class.new(user) }

  # Set the section's baseline so freshly-created content reads as "new" (past)
  # or already-seen (future).
  def seen(section, at)
    user.mark_section_seen!(section, at: at)
  end

  describe "#dot?" do
    it "fails closed for a nil user" do
      expect(described_class.new(nil).dot?(:mail)).to be false
    end

    it "is false for an unknown section" do
      expect(attention.dot?(:bogus)).to be false
    end

    it "memoizes within the instance" do
      seen(:documents, 1.hour.ago)
      expect(attention.dot?(:documents)).to be false

      create(:document, workspace: user.workspace)

      # Same instance keeps the cached result; a fresh one recomputes.
      expect(attention.dot?(:documents)).to be false
      expect(described_class.new(user).dot?(:documents)).to be true
    end
  end

  describe ":mail" do
    let(:account) { create(:email_account, workspace: user.workspace) }

    before { create(:email_account_user, user: user, email_account: account, can_read: true) }

    it "lights up for mail received since the last visit" do
      seen(:mail, 1.hour.ago)
      create(:email_message, email_account: account, received_at: Time.current)
      expect(attention.dot?(:mail)).to be true
    end

    it "stays clear when the last visit is newer than the mail" do
      create(:email_message, email_account: account, received_at: 2.hours.ago)
      seen(:mail, 1.hour.ago)
      expect(attention.dot?(:mail)).to be false
    end

    it "ignores mail on accounts the user cannot read" do
      other = create(:email_account, workspace: user.workspace)
      seen(:mail, 1.hour.ago)
      create(:email_message, email_account: other, received_at: Time.current)
      expect(attention.dot?(:mail)).to be false
    end
  end

  describe ":calendar" do
    it "lights up for an event added since the last visit" do
      account = create(:calendar_account, workspace: user.workspace)
      create(:calendar_account_user, user: user, calendar_account: account, can_read: true)
      calendar = create(:calendar, calendar_account: account)
      seen(:calendar, 1.hour.ago)
      create(:calendar_event, calendar: calendar)
      expect(attention.dot?(:calendar)).to be true
    end

    it "lights up for a new pending reminder" do
      seen(:calendar, 1.hour.ago)
      create(:reminder, workspace: user.workspace)
      expect(attention.dot?(:calendar)).to be true
    end

    it "stays clear with no new events or reminders" do
      seen(:calendar, 1.hour.ago)
      expect(attention.dot?(:calendar)).to be false
    end
  end

  describe ":documents" do
    it "lights up for a document added since the last visit" do
      seen(:documents, 1.hour.ago)
      create(:document, workspace: user.workspace)
      expect(attention.dot?(:documents)).to be true
    end

    it "ignores documents in another workspace" do
      seen(:documents, 1.hour.ago)
      create(:document)
      expect(attention.dot?(:documents)).to be false
    end
  end

  describe ":scout" do
    let(:thread) { create(:agent_thread, user: user) }

    it "lights up for a new AI reply" do
      seen(:scout, 1.hour.ago)
      create(:agent_message, agent_thread: thread, author_type: :ai)
      expect(attention.dot?(:scout)).to be true
    end

    it "ignores the user's own messages" do
      seen(:scout, 1.hour.ago)
      create(:agent_message, agent_thread: thread, author_type: :user)
      expect(attention.dot?(:scout)).to be false
    end

    it "ignores AI messages in setup_chat threads" do
      setup = create(:agent_thread, user: user, purpose: :setup_chat)
      seen(:scout, 1.hour.ago)
      create(:agent_message, agent_thread: setup, author_type: :ai)
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

    it "lights up for an active feed item created since the last visit" do
      seen(:home, 1.hour.ago)
      feed_item
      expect(attention.dot?(:home)).to be true
    end

    it "ignores dismissed or acted items" do
      seen(:home, 1.hour.ago)
      feed_item(dismissed_at: Time.current)
      feed_item(acted_at: Time.current)
      expect(attention.dot?(:home)).to be false
    end
  end
end
