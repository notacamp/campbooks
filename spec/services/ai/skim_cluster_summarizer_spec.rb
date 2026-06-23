require "rails_helper"

RSpec.describe Ai::SkimClusterSummarizer do
  MailStub = Struct.new(:from_address, :subject, :summary, keyword_init: true) unless defined?(MailStub)

  def mail(subject, from: "no-reply@github.com", summary: nil)
    MailStub.new(from_address: from, subject: subject, summary: summary)
  end

  let(:emails) { [ mail("PR #1 merged", summary: "merged"), mail("CI passed on main") ] }

  # Stub the resolved text model so no real API call is made.
  def stub_text_model(returns: nil, raises: nil)
    adapter = double("adapter")
    if raises
      allow(adapter).to receive(:chat).and_raise(raises)
    else
      allow(adapter).to receive(:chat).and_return(returns)
    end
    allow(Ai::Configuration).to receive(:for_any).and_return({ adapter: adapter, model: "m" })
    adapter
  end

  it "returns the model's one-sentence summary" do
    stub_text_model(returns: "Mostly GitHub CI notifications — nothing needs you.")
    expect(described_class.new(emails).summary).to eq("Mostly GitHub CI notifications — nothing needs you.")
  end

  it "collapses newlines and unwraps surrounding quotes" do
    stub_text_model(returns: %(  "Routine build\n  notifications."  ))
    expect(described_class.new(emails).summary).to eq("Routine build notifications.")
  end

  it "clamps an overly long summary" do
    stub_text_model(returns: "x" * 300)
    result = described_class.new(emails).summary
    expect(result.length).to be <= Ai::SkimClusterSummarizer::MAX_LEN
    expect(result).to end_with("…")
  end

  it "sends the cluster's subjects to the model" do
    adapter = double("adapter")
    allow(Ai::Configuration).to receive(:for_any).and_return({ adapter: adapter, model: "m" })
    expect(adapter).to receive(:chat) do |**kwargs|
      content = kwargs[:messages].first[:content]
      expect(content).to include("PR #1 merged").and include("CI passed on main")
      "ok"
    end
    described_class.new(emails).summary
  end

  it "returns nil when no text model is configured" do
    allow(Ai::Configuration).to receive(:for_any).and_return(nil)
    expect(described_class.new(emails).summary).to be_nil
  end

  it "never raises — returns nil when the adapter errors" do
    stub_text_model(raises: StandardError.new("boom"))
    expect(described_class.new(emails).summary).to be_nil
  end

  it "returns nil for an empty cluster (no model call)" do
    expect(Ai::Configuration).not_to receive(:for_any)
    expect(described_class.new([]).summary).to be_nil
  end

  it "returns nil when the model replies with blank text" do
    stub_text_model(returns: "   \n  ")
    expect(described_class.new(emails).summary).to be_nil
  end
end
