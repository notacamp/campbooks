require "rails_helper"

RSpec.describe Tools::DraftReply do
  let(:workspace) { create(:workspace) }
  let(:account) { create(:email_account, workspace: workspace) }
  let(:message) do
    create(:email_message, email_account: account, subject: "Quote", from_address: "client@example.com", body: "Can you confirm the price?")
  end

  # Capture the system prompt handed to the model, and return a canned draft.
  def stub_adapter(returning: '{"subject":"Re: Quote","body":"Confirmed."}')
    captured = {}
    adapter = double("adapter")
    allow(adapter).to receive(:chat) do |args|
      captured[:system] = args[:system]
      returning
    end
    allow(Ai::Configuration).to receive(:for).with("draft_reply").and_return(
      { adapter: adapter, model: "test-model", max_tokens: 500, temperature: 0.5 }
    )
    captured
  end

  it "injects the user's writing style into the system prompt" do
    user = create(:user, workspace: workspace, writing_style: "I sign off as Sam and keep replies breezy.")
    captured = stub_adapter
    Tools::DraftReply.call(message, {}, user: user)
    expect(captured[:system]).to include("How #{user.name} writes")
    expect(captured[:system]).to include("I sign off as Sam")
  end

  it "includes the auto-learned profile when present" do
    user = create(:user, workspace: workspace, writing_style_learned: "Greets with a first name, signs off Cheers.")
    captured = stub_adapter
    Tools::DraftReply.call(message, {}, user: user)
    expect(captured[:system]).to include("Greets with a first name")
  end

  it "omits the writing-style block when the user has none" do
    user = create(:user, workspace: workspace)
    captured = stub_adapter
    Tools::DraftReply.call(message, {}, user: user)
    expect(captured[:system]).not_to include("How #{user.name} writes")
  end

  it "returns the drafted subject and body" do
    user = create(:user, workspace: workspace)
    stub_adapter
    result = Tools::DraftReply.call(message, {}, user: user)
    expect(result[:draft]["subject"]).to eq("Re: Quote")
    expect(result[:draft]["body"]).to include("Confirmed.")
  end
end
