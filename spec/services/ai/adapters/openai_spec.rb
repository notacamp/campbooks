require "rails_helper"

RSpec.describe Ai::Adapters::Openai do
  describe "#embed" do
    let(:adapter) { described_class.new(api_key: "test-key") }
    let(:request) { double("request") }
    let(:response) do
      double("response", body: { "data" => [ { "index" => 0, "embedding" => [ 0.1, 0.2 ] } ] }.to_json)
    end
    let(:connection) { double("connection") }

    before do
      allow(adapter).to receive(:connection).and_return(connection)
      allow(request).to receive(:body=)
      allow(connection).to receive(:post).and_yield(request).and_return(response)
    end

    it "caps each input to the safe character budget so it can't exceed the token limit (the 400 cause)" do
      adapter.embed("x" * 50_000)

      expect(request).to have_received(:body=) do |json|
        expect(JSON.parse(json)["input"].first.length).to eq(described_class::EMBED_MAX_INPUT_CHARS)
      end
    end

    it "leaves a normal-sized input untouched" do
      adapter.embed("short text")

      expect(request).to have_received(:body=) do |json|
        expect(JSON.parse(json)["input"]).to eq([ "short text" ])
      end
    end
  end
end
