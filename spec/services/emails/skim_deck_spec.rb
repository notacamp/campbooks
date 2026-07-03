require "rails_helper"

RSpec.describe Emails::SkimDeck do
  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace) }
  let(:account)   { create(:email_account, workspace: workspace, email_address: "me@example.com") }

  before do
    create(:email_account_user, user: user, email_account: account, can_read: true)
    allow(Emails::InboxFolders).to receive(:ids_for).and_return([ "INBOX" ])
  end

  def inbox_email(thread, from:, at: 1.hour.ago)
    create(:email_message, email_account: account, email_thread: thread,
           from_address: from, provider_folder_id: "INBOX", received_at: at, skimmed_at: nil)
  end

  def rings        = described_class.for(user)
  def all_email_ids(rings) = rings.flat_map { |r| r[:clusters] }.flat_map { |c| c[:email_ids] }
  def follow_ups(rings)    = rings.find { |r| r[:theme] == :follow_ups }

  it "hides a conversation the owner already answered (holds the last word, no follow-up due)" do
    answered = create(:email_thread, email_account: account, last_outbound_at: 30.minutes.ago, last_inbound_at: 2.hours.ago)
    msg = inbox_email(answered, from: "dana@acme.com", at: 2.hours.ago)

    expect(all_email_ids(rings)).not_to include(msg.id)
  end

  it "keeps a conversation still awaiting the owner's reply" do
    open_thread = create(:email_thread, email_account: account, last_outbound_at: nil, last_inbound_at: 1.hour.ago)
    msg = inbox_email(open_thread, from: "dana@acme.com")

    expect(all_email_ids(rings)).to include(msg.id)
  end

  it "surfaces a due follow-up in a leading Follow-ups ring" do
    due = create(:email_thread, email_account: account,
                 last_outbound_at: 4.days.ago, last_inbound_at: 5.days.ago,
                 follow_up_expected: true, follow_up_at: 1.day.ago, follow_up_reason: "You asked them to confirm")
    msg = inbox_email(due, from: "dana@acme.com", at: 5.days.ago)

    result = rings
    expect(result.first[:theme]).to eq(:follow_ups)
    expect(all_email_ids(result)).to include(msg.id)
    card = follow_ups(result)[:clusters].first
    expect(card[:follow_up]).to be(true)
    expect(card[:follow_up_reason]).to eq("You asked them to confirm")
  end

  it "surfaces a due follow-up even when it sits outside the 14-day recency window" do
    old_due = create(:email_thread, email_account: account,
                     last_outbound_at: 20.days.ago, last_inbound_at: 25.days.ago,
                     follow_up_expected: true, follow_up_at: 15.days.ago, follow_up_reason: "Still waiting")
    msg = inbox_email(old_due, from: "dana@acme.com", at: 25.days.ago) # older than SkimScope::WINDOW

    expect(all_email_ids(rings)).to include(msg.id)
  end

  it "surfaces a silent thread with no AI verdict once past the heuristic threshold (de-gated)" do
    thread = create(:email_thread, email_account: account, last_outbound_at: 4.days.ago, last_inbound_at: 5.days.ago)
    msg = inbox_email(thread, from: "dana@acme.com", at: 5.days.ago)

    result = rings
    expect(result.first[:theme]).to eq(:follow_ups)
    expect(all_email_ids(result)).to include(msg.id)
  end
end
