require "rails_helper"

RSpec.describe Ai::CommitmentMatcher do
  # Stub Ai::Configuration.for_any for the duration of the block — mirrors the
  # pattern from spec/services/ai/task_extractor_spec.rb.
  def with_config(adapter)
    allow(Ai::Configuration).to receive(:for_any).and_return({ adapter: adapter, model: "m" })
    yield
  end

  let(:workspace) { create(:workspace) }
  let(:source)    { create(:document, workspace: workspace) }

  let(:reminder) do
    create(:reminder, workspace: workspace, source: source,
           reminder_type: :payment_due, title: "Pay invoice",
           due_at: Time.zone.parse("2026-07-15 09:00:00"))
  end

  let(:neighbor) { Commitments::Neighbor.new(kind: "reminder", record: reminder) }

  let(:candidate) do
    { "kind" => "reminder", "title" => "Pay the invoice", "due_at" => "2026-07-15T09:00:00+00:00",
      "timed" => false, "reminder_type" => "payment_due" }
  end

  def matcher(neighbors = [ neighbor ])
    described_class.new(workspace: workspace, candidate: candidate, neighbors: neighbors)
  end

  describe "valid match within confidence threshold" do
    it "returns the matched neighbor's record when match_index is valid and confidence >= 0.8" do
      adapter = instance_double(Ai::Adapters::Openai)
      allow(adapter).to receive(:chat).and_return('{"match_index":1,"confidence":0.9,"reason":"same invoice"}')

      result = nil
      with_config(adapter) { result = matcher.match }

      expect(result).to eq(reminder)
    end

    it "sets failed? to false on a successful match" do
      adapter = instance_double(Ai::Adapters::Openai)
      allow(adapter).to receive(:chat).and_return('{"match_index":1,"confidence":0.9,"reason":"same"}')

      m = matcher
      with_config(adapter) { m.match }
      expect(m.failed?).to be(false)
    end
  end

  describe "confidence below the threshold" do
    it "returns nil when confidence is 0.5 (below 0.8)" do
      adapter = instance_double(Ai::Adapters::Openai)
      allow(adapter).to receive(:chat).and_return('{"match_index":1,"confidence":0.5,"reason":"weak"}')

      result = nil
      with_config(adapter) { result = matcher.match }

      expect(result).to be_nil
    end
  end

  describe "match_index out of range" do
    it "returns nil when match_index references a non-existent neighbor" do
      adapter = instance_double(Ai::Adapters::Openai)
      allow(adapter).to receive(:chat).and_return('{"match_index":99,"confidence":0.95,"reason":"oob"}')

      result = nil
      with_config(adapter) { result = matcher.match }

      expect(result).to be_nil
    end
  end

  describe "adapter raising a transient error" do
    it "returns nil and sets failed? true, and does not raise" do
      adapter = instance_double(Ai::Adapters::Openai)
      allow(adapter).to receive(:chat).and_raise(Faraday::TooManyRequestsError, "429")

      m = matcher
      expect {
        with_config(adapter) { m.match }
      }.not_to raise_error

      expect(m.match).to be_nil  # idempotent: still nil
      expect(m.failed?).to be(true)
    end
  end

  describe "no AI config" do
    it "returns nil and failed? is false when no config is available" do
      allow(Ai::Configuration).to receive(:for_any).and_return(nil)
      m = matcher
      expect(m.match).to be_nil
      expect(m.failed?).to be(false)
    end
  end

  describe "empty neighbors" do
    it "returns nil without calling the adapter when neighbors is empty" do
      expect(Ai::Configuration).not_to receive(:for_any)
      m = described_class.new(workspace: workspace, candidate: candidate, neighbors: [])
      expect(m.match).to be_nil
    end
  end
end
