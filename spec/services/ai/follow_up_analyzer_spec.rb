require "rails_helper"

RSpec.describe Ai::FollowUpAnalyzer do
  let(:reply) { build(:email_message, subject: "Re: Contract", body: "Can you confirm the start date?", received_at: Time.current) }

  def with_model_returning(json)
    adapter = instance_double("adapter")
    allow(adapter).to receive(:chat).and_return(json)
    allow(Ai::Configuration).to receive(:for_any).and_return({ adapter: adapter, model: "test" })
  end

  it "returns an expected follow-up with clamped days and the reason" do
    with_model_returning('{"follow_up_expected": true, "follow_up_in_days": 4, "reason": "You asked them to confirm the date", "confidence": 0.8}')
    result = described_class.new(reply: reply).analyze
    expect(result.expected).to be(true)
    expect(result.days).to eq(4)
    expect(result.reason).to eq("You asked them to confirm the date")
  end

  it "clamps an out-of-range day count to 1..30" do
    with_model_returning('{"follow_up_expected": true, "follow_up_in_days": 999, "reason": "x", "confidence": 0.9}')
    expect(described_class.new(reply: reply).analyze.days).to eq(30)
  end

  it "treats a low-confidence verdict as not expected" do
    with_model_returning('{"follow_up_expected": true, "follow_up_in_days": 3, "reason": "x", "confidence": 0.3}')
    expect(described_class.new(reply: reply).analyze.expected).to be(false)
  end

  it "returns a not-expected result when the model says no" do
    with_model_returning('{"follow_up_expected": false, "confidence": 0.2}')
    result = described_class.new(reply: reply).analyze
    expect(result.expected).to be(false)
    expect(result.days).to be_nil
  end

  it "returns nil when no AI text model is configured" do
    allow(Ai::Configuration).to receive(:for_any).and_return(nil)
    expect(described_class.new(reply: reply).analyze).to be_nil
  end

  it "never raises on a model error (returns nil)" do
    adapter = instance_double("adapter")
    allow(adapter).to receive(:chat).and_raise(StandardError, "boom")
    allow(Ai::Configuration).to receive(:for_any).and_return({ adapter: adapter, model: "test" })
    expect(described_class.new(reply: reply).analyze).to be_nil
  end
end
