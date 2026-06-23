require "rails_helper"

RSpec.describe Emails::FollowUpAnalysisJob do
  let(:account) { create(:email_account, email_address: "me@example.com") }
  let(:thread)  { create(:email_thread, email_account: account, last_outbound_at: 1.day.ago, last_inbound_at: 2.days.ago) }
  let!(:inbound)  { create(:email_message, email_account: account, email_thread: thread, from_address: "them@acme.com", received_at: 2.days.ago) }
  let!(:trigger)  { create(:email_message, email_account: account, email_thread: thread, from_address: "me@example.com", received_at: 1.day.ago) }

  let(:verdict) { Ai::FollowUpAnalyzer::Result.new(expected: true, days: 3, reason: "Confirm the date", confidence: 0.8) }

  before do
    allow(Ai::ProviderSetup).to receive(:configured?).and_return(true)
    allow(Feed::RefreshJob).to receive(:enqueue_for_workspace)
    allow(Ai::FollowUpAnalyzer).to receive(:new).and_return(instance_double(Ai::FollowUpAnalyzer, analyze: verdict))
  end

  it "stores the AI verdict on the thread" do
    described_class.new.perform(thread.id, trigger.id)

    thread.reload
    expect(thread.follow_up_expected?).to be(true)
    expect(thread.follow_up_reason).to eq("Confirm the date")
    expect(thread.follow_up_at).to be_within(1.minute).of(trigger.received_at + 3.days)
    expect(thread.follow_up_outbound_message_id).to eq(trigger.id)
  end

  it "is idempotent — skips when already analysed at/after this trigger" do
    thread.update_columns(follow_up_last_analyzed_at: trigger.received_at + 1.hour)
    described_class.new.perform(thread.id, trigger.id)
    expect(Ai::FollowUpAnalyzer).not_to have_received(:new)
  end

  it "skips when the other party already replied (owner no longer holds the last word)" do
    thread.update_columns(last_inbound_at: 1.minute.ago) # inbound now newer than outbound
    described_class.new.perform(thread.id, trigger.id)
    expect(Ai::FollowUpAnalyzer).not_to have_received(:new)
  end

  it "skips when no AI text model is configured" do
    allow(Ai::ProviderSetup).to receive(:configured?).and_return(false)
    described_class.new.perform(thread.id, trigger.id)
    expect(Ai::FollowUpAnalyzer).not_to have_received(:new)
  end

  it "skips an automated / no-reply counterparty" do
    inbound.update_columns(from_address: "no-reply@acme.com")
    described_class.new.perform(thread.id, trigger.id)
    expect(Ai::FollowUpAnalyzer).not_to have_received(:new)
  end

  it "clears the verdict when the AI says no follow-up is expected" do
    allow(Ai::FollowUpAnalyzer).to receive(:new).and_return(
      instance_double(Ai::FollowUpAnalyzer, analyze: Ai::FollowUpAnalyzer::Result.new(expected: false, days: nil, reason: nil, confidence: 0.2))
    )
    described_class.new.perform(thread.id, trigger.id)
    thread.reload
    expect(thread.follow_up_expected?).to be(false)
    expect(thread.follow_up_last_analyzed_at).to be_present
  end
end
