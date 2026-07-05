require "rails_helper"

RSpec.describe Feed::Sources::FollowUp do
  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace) }
  let(:account)   { create(:email_account, workspace: workspace, email_address: "me@example.com") }
  subject(:source) { described_class.new(user) }

  before do
    create(:email_account_user, user: user, email_account: account, can_read: true)
    allow(Emails::InboxFolders).to receive(:ids_for).and_return([ "INBOX" ])
  end

  def due_thread
    create(:email_thread, email_account: account,
           last_outbound_at: 4.days.ago, last_inbound_at: 5.days.ago,
           follow_up_last_analyzed_at: 4.days.ago,
           follow_up_expected: true, follow_up_at: 1.day.ago, follow_up_reason: "Confirm the date")
  end

  describe "#candidates" do
    it "emits one candidate per due thread, keyed on the thread, addressed to the other party" do
      thread = due_thread
      sent = create(:email_message, email_account: account, email_thread: thread, from_address: "me@example.com",
                    subject: "Q3 proposal", body: "Hi Dana, attaching the Q3 proposal.", provider_folder_id: "SENT", received_at: 4.days.ago)
      inbound = create(:email_message, email_account: account, email_thread: thread, from_address: "dana@acme.com", provider_folder_id: "INBOX", received_at: 5.days.ago)

      candidates = source.candidates
      expect(candidates.size).to eq(1)
      c = candidates.first
      # Anchored to the inbound message — that's what keeps the action addressed to
      # the other party and subject to inbox/admission gating.
      expect(c[:subject]).to eq(inbound)
      expect(c[:dedupe_key]).to eq("follow_up:#{thread.id}")
      expect(c[:attention]).to be(true)
      expect(c[:data]["reason"]).to eq("Confirm the date")
      # ...but the card SHOWS the mail the user sent and is chasing, not the inbound.
      expect(c[:data]["sent_subject"]).to eq("Q3 proposal")
      expect(c[:data]["sent_message_id"]).to eq(sent.id)
    end

    it "ignores threads that are not due (within grace, dismissed, or the AI said wait / no)" do
      [ # still within the grace window (and under the heuristic floor)
        { last_outbound_at: 1.hour.ago, last_inbound_at: 2.hours.ago },
        # the owner dismissed the nudge
        { last_outbound_at: 4.days.ago, last_inbound_at: 5.days.ago, follow_up_dismissed_at: Time.current },
        # the AI analysed it and scheduled the nudge for the future
        { last_outbound_at: 4.days.ago, last_inbound_at: 5.days.ago,
          follow_up_last_analyzed_at: 4.days.ago, follow_up_expected: true, follow_up_at: 1.day.from_now },
        # the AI analysed it and judged no follow-up is warranted
        { last_outbound_at: 4.days.ago, last_inbound_at: 5.days.ago,
          follow_up_last_analyzed_at: 4.days.ago, follow_up_expected: false } ].each do |attrs|
        t = create(:email_thread, { email_account: account }.merge(attrs))
        create(:email_message, email_account: account, email_thread: t, from_address: "dana@acme.com", provider_folder_id: "INBOX", received_at: 5.days.ago)
      end
      expect(source.candidates).to be_empty
    end

    it "surfaces a silent thread with no AI verdict once past the heuristic threshold (de-gated)" do
      thread = create(:email_thread, email_account: account, last_outbound_at: 4.days.ago, last_inbound_at: 5.days.ago)
      create(:email_message, email_account: account, email_thread: thread, from_address: "me@example.com", provider_folder_id: "INBOX", received_at: 4.days.ago)
      create(:email_message, email_account: account, email_thread: thread, from_address: "dana@acme.com", provider_folder_id: "INBOX", received_at: 5.days.ago)
      expect(source.candidates.size).to eq(1)
    end

    it "skips a thread whose only messages are the owner's (no one to nudge)" do
      thread = due_thread
      create(:email_message, email_account: account, email_thread: thread, from_address: "me@example.com", provider_folder_id: "INBOX", received_at: 4.days.ago)
      expect(source.candidates).to be_empty
    end
  end

  describe "anchoring and silence scoring" do
    def thread_with_messages(**attrs)
      thread = create(:email_thread, { email_account: account,
                                       last_outbound_at: 4.days.ago, last_inbound_at: 5.days.ago }.merge(attrs))
      create(:email_message, email_account: account, email_thread: thread,
             from_address: "me@example.com", provider_folder_id: "INBOX", received_at: thread.last_outbound_at)
      create(:email_message, email_account: account, email_thread: thread,
             from_address: "dana@acme.com", provider_folder_id: "INBOX", received_at: thread.last_inbound_at)
      thread
    end

    it "anchors sort_at on the AI nudge time when present" do
      thread = thread_with_messages(follow_up_last_analyzed_at: 4.days.ago,
                                    follow_up_expected: true, follow_up_at: 1.day.ago)

      expect(source.candidates.first[:sort_at]).to be_within(1.second).of(thread.follow_up_at)
    end

    it "anchors an un-analyzed thread on the send moment — never the run time" do
      thread = thread_with_messages(last_outbound_at: 10.days.ago, last_inbound_at: 11.days.ago)

      expect(source.candidates.first[:sort_at]).to be_within(1.second).of(thread.last_outbound_at)
    end

    it "scores deeper silence higher, up to the climb ceiling" do
      fresh = thread_with_messages
      stale = thread_with_messages(last_outbound_at: 20.days.ago, last_inbound_at: 21.days.ago)

      by_key = source.candidates.index_by { |c| c[:dedupe_key] }
      fresh_score = by_key["follow_up:#{fresh.id}"][:score]
      stale_score = by_key["follow_up:#{stale.id}"][:score]

      expect(fresh_score).to be_within(1).of(71) # 68 + 4 days of climb
      expect(stale_score).to eq(described_class::BASE_SCORE + described_class::SILENCE_CLIMB)
    end
  end

  describe "#still_valid?" do
    let(:thread) { due_thread }
    let(:inbound) { create(:email_message, email_account: account, email_thread: thread, from_address: "dana@acme.com", provider_folder_id: "INBOX") }

    it "is valid while the follow-up is pending and the mail is in the inbox" do
      expect(source.still_valid?(double("item"), inbound)).to be_truthy
    end

    it "drops once the follow-up is dismissed on the thread" do
      thread.update_columns(follow_up_dismissed_at: Time.current)
      expect(source.still_valid?(double("item"), inbound.reload)).to be_falsey
    end

    it "drops once the mail leaves the inbox" do
      inbound.update_columns(provider_folder_id: "ARCHIVE")
      expect(source.still_valid?(double("item"), inbound)).to be_falsey
    end
  end
end
