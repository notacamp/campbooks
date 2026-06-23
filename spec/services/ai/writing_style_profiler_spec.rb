require "rails_helper"

RSpec.describe Ai::WritingStyleProfiler do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }
  let(:account) { create(:email_account, workspace: workspace, email_address: "sam@example.com") }

  before { create(:email_account_user, :collaborator, user: user, email_account: account) }

  def stub_adapter(returning:)
    adapter = double("adapter")
    allow(adapter).to receive(:chat).and_return(returning)
    allow(Ai::Configuration).to receive(:for).with("draft_reply").and_return(
      { adapter: adapter, model: "m", max_tokens: 500, temperature: 0.3 }
    )
  end

  it "samples only the user's sent mail and strips HTML" do
    create(:email_message, email_account: account, from_address: "sam@example.com", body: "<p>Hi Jo, sounds good. Cheers, Sam</p>")
    create(:email_message, email_account: account, from_address: "inbound@example.com", body: "not sent by Sam")

    samples = described_class.sent_samples(user)

    expect(samples.size).to eq(1)
    expect(samples.first).to include("Hi Jo")
    expect(samples.first).not_to include("<p>")
  end

  it "writes a learned profile and stamps the time" do
    create(:email_message, email_account: account, from_address: "sam@example.com", body: "Hi Jo, sounds good. Cheers, Sam")
    stub_adapter(returning: "Greets by first name; signs off Cheers, Sam.")

    expect(described_class.call(user)).to include("Cheers, Sam")
    expect(user.reload.writing_style_learned).to include("Cheers, Sam")
    expect(user.writing_style_updated_at).to be_present
  end

  it "no-ops when there is no sent mail" do
    expect(described_class.call(user)).to be_nil
    expect(user.reload.writing_style_learned).to be_nil
  end
end
