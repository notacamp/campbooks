require "rails_helper"

RSpec.describe Labels::AiClassifier, type: :service do
  let(:workspace) { create(:workspace) }
  let(:account)   { create(:email_account, workspace: workspace, provider: :google) }
  let(:tag) do
    Tag.create!(workspace: workspace, email_account: account, source: :external,
                external_label_id: "L1", name: "Mailing Lists", color: "#ccc")
  end

  let(:claude_client) { instance_double(Anthropic::Client) }

  # Force the self-hosted legacy Anthropic path so we can stub one client.
  def stub_ai(text)
    allow(Ai::Configuration).to receive(:for_any).and_return(nil)
    allow(Ai::LegacyFallback).to receive(:allowed?).and_return(true)
    allow(Anthropic::Client).to receive(:new).and_return(claude_client)
    allow(claude_client).to receive_message_chain(:messages, :create).and_return(
      double("messages", content: [ double("block", type: "text", text: text) ])
    )
  end

  it "keeps a high-confidence user_tag visible" do
    stub_ai('{"verdict":"user_tag","confidence":0.95,"reason":"client folder"}')
    result = described_class.new(tag).classify
    expect(result).to include(kind: :user, hidden: false)
    expect(result[:confidence]).to eq(0.95)
  end

  it "hides a low_value verdict" do
    stub_ai('{"verdict":"low_value","confidence":0.9,"reason":"noise"}')
    expect(described_class.new(tag).classify).to include(kind: :low_value, hidden: true)
  end

  it "hides a low-confidence user_tag (aggressive threshold)" do
    stub_ai('{"verdict":"user_tag","confidence":0.4,"reason":"unsure"}')
    expect(described_class.new(tag).classify).to include(kind: :low_value, hidden: true)
  end

  it "strips markdown code fences before parsing" do
    stub_ai("```json\n{\"verdict\":\"user_tag\",\"confidence\":0.8}\n```")
    expect(described_class.new(tag).classify).to include(kind: :user, hidden: false)
  end

  it "returns nil on invalid JSON" do
    stub_ai("not json at all")
    expect(described_class.new(tag).classify).to be_nil
  end

  it "returns nil when no provider is available" do
    allow(Ai::Configuration).to receive(:for_any).and_return(nil)
    allow(Ai::LegacyFallback).to receive(:allowed?).and_return(false)
    expect(described_class.new(tag).classify).to be_nil
  end
end
