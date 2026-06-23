require "rails_helper"

RSpec.describe Tools::DraftFollowUp do
  let(:account) { create(:email_account, email_address: "me@example.com") }
  let(:thread)  { create(:email_thread, email_account: account, follow_up_reason: "Confirm the date") }
  let!(:inbound) { create(:email_message, email_account: account, email_thread: thread, from_address: "dana@acme.com", subject: "Budget", received_at: 5.days.ago) }
  let!(:my_reply) { create(:email_message, email_account: account, email_thread: thread, from_address: "me@example.com", body: "Can you confirm the start date?", received_at: 4.days.ago) }

  it "returns a draft built from the model output, addressed via the thread's inbound message" do
    allow(Tools::DraftReply).to receive(:call_ai).and_return('{"subject":"Re: Budget","body":"Just following up on my note about the start date."}')

    result = described_class.call(inbound, {}, user: nil)

    expect(result[:draft]["subject"]).to eq("Re: Budget")
    expect(result[:draft]["body"]).to include("following up")
  end

  it "feeds the user's own earlier reply (not the inbound message) to the model" do
    captured = nil
    allow(Tools::DraftReply).to receive(:call_ai) { |_system, user_msg| captured = user_msg; '{"subject":"Re: Budget","body":"x"}' }

    described_class.call(inbound, {}, user: nil)

    expect(captured).to include("confirm the start date") # from my_reply, the message we follow up on
    expect(captured).to include("Confirm the date")       # the AI reason carried on the thread
  end

  it "returns nil when the model produces nothing" do
    allow(Tools::DraftReply).to receive(:call_ai).and_return(nil)
    expect(described_class.call(inbound, {}, user: nil)).to be_nil
  end
end
