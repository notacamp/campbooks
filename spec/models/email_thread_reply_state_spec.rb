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

  describe ".holds_last_word (scope)" do
    it "matches threads the owner sent last (incl. cold sends), not those the other party answered" do
      held   = create(:email_thread, email_account: account, last_outbound_at: 1.hour.ago, last_inbound_at: 2.hours.ago)
      cold   = create(:email_thread, email_account: account, last_outbound_at: 1.hour.ago, last_inbound_at: nil)
      theirs = create(:email_thread, email_account: account, last_outbound_at: 2.hours.ago, last_inbound_at: 1.hour.ago)
      never  = create(:email_thread, email_account: account, last_outbound_at: nil, last_inbound_at: 1.hour.ago)

      result = EmailThread.holds_last_word
      expect(result).to include(held, cold)
      expect(result).not_to include(theirs, never)
    end
  end

  describe ".awaiting_reply (scope)" do
    it "matches held threads past the grace window, excluding dismissed and too-recent sends" do
      due       = create(:email_thread, email_account: account, last_outbound_at: 1.day.ago, last_inbound_at: 2.days.ago)
      recent    = create(:email_thread, email_account: account, last_outbound_at: 1.hour.ago, last_inbound_at: 2.hours.ago)
      dismissed = create(:email_thread, email_account: account, last_outbound_at: 1.day.ago, last_inbound_at: 2.days.ago, follow_up_dismissed_at: Time.current)

      result = EmailThread.awaiting_reply
      expect(result).to include(due)
      expect(result).not_to include(recent, dismissed)
    end

    it "vets with the AI verdict: drops 'no follow-up expected', keeps confirmed and unjudged" do
      unjudged  = create(:email_thread, email_account: account, last_outbound_at: 1.day.ago, last_inbound_at: 2.days.ago)
      confirmed = create(:email_thread, email_account: account, last_outbound_at: 1.day.ago, last_inbound_at: 2.days.ago,
                         follow_up_last_analyzed_at: 1.day.ago, follow_up_expected: true)
      fyi       = create(:email_thread, email_account: account, last_outbound_at: 1.day.ago, last_inbound_at: 2.days.ago,
                         follow_up_last_analyzed_at: 1.day.ago, follow_up_expected: false)

      result = EmailThread.awaiting_reply
      expect(result).to include(unjudged, confirmed)
      expect(result).not_to include(fyi)
    end
  end
end
