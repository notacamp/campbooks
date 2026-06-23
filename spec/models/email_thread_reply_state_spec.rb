require "rails_helper"

# Denormalized reply-state behaviour added for the follow-up feature.
RSpec.describe EmailThread, "reply state" do
  let(:account) { create(:email_account) }

  describe "#holds_last_word?" do
    it "is true when the owner replied after the other party (out >= in)" do
      thread = build(:email_thread, email_account: account, last_outbound_at: 1.hour.ago, last_inbound_at: 2.hours.ago)
      expect(thread.holds_last_word?).to be(true)
    end

    it "is true for an outbound-only thread (cold send)" do
      thread = build(:email_thread, email_account: account, last_outbound_at: 1.hour.ago, last_inbound_at: nil)
      expect(thread.holds_last_word?).to be(true)
    end

    it "is false when the other party replied last (in > out)" do
      thread = build(:email_thread, email_account: account, last_outbound_at: 2.hours.ago, last_inbound_at: 1.hour.ago)
      expect(thread.holds_last_word?).to be(false)
    end

    it "is false when the owner never replied" do
      thread = build(:email_thread, email_account: account, last_outbound_at: nil, last_inbound_at: 1.hour.ago)
      expect(thread.holds_last_word?).to be(false)
    end
  end

  describe "#follow_up_due? and .follow_up_due" do
    it "is due when expected, undismissed and the time has passed" do
      thread = create(:email_thread, email_account: account,
                      follow_up_expected: true, follow_up_at: 1.hour.ago)
      expect(thread.follow_up_due?).to be(true)
      expect(EmailThread.follow_up_due).to include(thread)
    end

    it "is not due before its time" do
      thread = create(:email_thread, email_account: account,
                      follow_up_expected: true, follow_up_at: 1.hour.from_now)
      expect(thread.follow_up_due?).to be(false)
      expect(EmailThread.follow_up_due).not_to include(thread)
    end

    it "is not due once dismissed" do
      thread = create(:email_thread, email_account: account,
                      follow_up_expected: true, follow_up_at: 1.hour.ago, follow_up_dismissed_at: Time.current)
      expect(thread.follow_up_due?).to be(false)
      expect(EmailThread.follow_up_due).not_to include(thread)
    end
  end
end
