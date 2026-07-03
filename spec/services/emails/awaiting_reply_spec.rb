require "rails_helper"

RSpec.describe Emails::AwaitingReply do
  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace) }
  let(:account)   { create(:email_account, workspace: workspace, email_address: "me@example.com") }

  before do
    create(:email_account_user, user: user, email_account: account, can_read: true)
    allow(Emails::InboxFolders).to receive(:ids_for).and_return([ "INBOX" ])
  end

  # A thread the owner replied to last (holds the last word), with the other
  # party's message sitting in the inbox. sent_ago/replied_ago set the
  # denormalized reply-state columns the scope reads. Returns the thread.
  def waiting_thread(sent_ago: 4.days.ago, replied_ago: 5.days.ago, counterparty: "dana@acme.com", **attrs)
    thread = create(:email_thread,
                    { email_account: account, last_outbound_at: sent_ago, last_inbound_at: replied_ago }.merge(attrs))
    create(:email_message, email_account: account, email_thread: thread,
           from_address: "me@example.com", provider_folder_id: "INBOX", received_at: sent_ago)
    create(:email_message, email_account: account, email_thread: thread,
           from_address: counterparty, provider_folder_id: "INBOX", received_at: replied_ago)
    thread
  end

  # NOTE: build the records BEFORE touching `query` — it memoizes its result on
  # first read, so a thread created after that first read wouldn't be seen.
  subject(:query) { described_class.new(user) }

  describe "#threads (the durable, AI-free list)" do
    it "includes a thread the owner replied to last, past the grace window" do
      thread = waiting_thread
      expect(query.threads).to include(thread)
    end

    it "excludes a thread where the other party replied last" do
      waiting_thread(sent_ago: 5.days.ago, replied_ago: 4.days.ago) # in > out
      expect(query.threads).to be_empty
    end

    it "excludes a reply sent within the grace window" do
      waiting_thread(sent_ago: 1.hour.ago, replied_ago: 2.hours.ago)
      expect(query.threads).to be_empty
    end

    it "excludes a dismissed thread" do
      waiting_thread(follow_up_dismissed_at: Time.current)
      expect(query.threads).to be_empty
    end

    it "excludes a thread whose counterparty is a no-reply address" do
      waiting_thread(counterparty: "no-reply@acme.com")
      expect(query.threads).to be_empty
    end

    it "excludes threads from accounts the user cannot read" do
      other = create(:email_account, workspace: workspace, email_address: "other@example.com")
      thread = create(:email_thread, email_account: other, last_outbound_at: 4.days.ago, last_inbound_at: 5.days.ago)
      create(:email_message, email_account: other, email_thread: thread,
             from_address: "x@acme.com", provider_folder_id: "INBOX", received_at: 5.days.ago)
      expect(query.threads).to be_empty
    end
  end

  describe "#count" do
    it "counts the waiting threads" do
      2.times { waiting_thread }
      expect(query.count).to eq(2)
    end
  end

  describe "#due (the proactive subset — AI as enhancer, not gatekeeper)" do
    it "includes a silent thread with no AI verdict once past the heuristic threshold" do
      thread = waiting_thread(sent_ago: 4.days.ago)
      expect(query.due).to include(thread)
    end

    it "keeps a not-yet-aged thread in the list but out of the nudge set" do
      thread = waiting_thread(sent_ago: 1.day.ago) # past 6h grace, under the 3-day floor
      expect(query.threads).to include(thread)
      expect(query.due).not_to include(thread)
    end

    it "honours an AI verdict: due when analysed, expected and the nudge time has passed" do
      thread = waiting_thread(sent_ago: 8.hours.ago,
                              follow_up_last_analyzed_at: 7.hours.ago,
                              follow_up_expected: true, follow_up_at: 1.hour.ago)
      expect(query.due).to include(thread)
    end

    it "honours an AI verdict: not due when it scheduled the nudge for later" do
      thread = waiting_thread(sent_ago: 4.days.ago,
                              follow_up_last_analyzed_at: 4.days.ago,
                              follow_up_expected: true, follow_up_at: 1.day.from_now)
      expect(query.threads).to include(thread) # still waiting…
      expect(query.due).not_to include(thread) # …but the AI said wait
    end

    it "drops a thread the AI judged as not expecting a reply (FYI / closing)" do
      thread = waiting_thread(sent_ago: 4.days.ago,
                              follow_up_last_analyzed_at: 4.days.ago,
                              follow_up_expected: false)
      expect(query.threads).not_to include(thread) # AI vetted it out of the list…
      expect(query.due).not_to include(thread)     # …and out of the nudge set
    end
  end
end
