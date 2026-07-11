# frozen_string_literal: true

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

    it "caps each input to EMBED_MAX_INPUT_CHARS" do
      adapter.embed("x" * 50_000, model: "text-embedding-3-small")

      expect(request).to have_received(:body=) do |json|
        expect(JSON.parse(json)["input"].first.length).to eq(described_class::EMBED_MAX_INPUT_CHARS)
      end
    end

    it "leaves a normal-sized input untouched" do
      adapter.embed("short text", model: "text-embedding-3-small")

      expect(request).to have_received(:body=) do |json|
        expect(JSON.parse(json)["input"]).to eq([ "short text" ])
      end
    end

    it "does NOT include dimensions key when dimensions is nil" do
      adapter.embed("hello", model: "text-embedding-3-small", dimensions: nil)

      expect(request).to have_received(:body=) do |json|
        expect(JSON.parse(json)).not_to have_key("dimensions")
      end
    end

    it "includes dimensions key when dimensions is provided" do
      adapter.embed("hello", model: "text-embedding-3-small", dimensions: 512)

      expect(request).to have_received(:body=) do |json|
        expect(JSON.parse(json)["dimensions"]).to eq(512)
      end
    end

    it "returns a single vector when input is a scalar string" do
      result = adapter.embed("hello", model: "text-embedding-3-small")
      expect(result).to eq([ 0.1, 0.2 ])
    end

    it "returns an array of vectors when input is an array" do
      result = adapter.embed([ "hello" ], model: "text-embedding-3-small")
      expect(result).to eq([ [ 0.1, 0.2 ] ])
    end
  end

  describe "#embeddings_endpoint" do
    it "returns the standard OpenAI endpoint when using the default chat endpoint" do
      adapter = described_class.new(api_key: "k")
      expect(adapter.send(:embeddings_endpoint)).to eq("https://api.openai.com/v1/embeddings")
    end

    it "derives /embeddings from a custom /chat/completions endpoint" do
      adapter = described_class.new(api_key: "k",
                                    endpoint_url: "https://custom.example.com/v1/chat/completions")
      expect(adapter.send(:embeddings_endpoint)).to eq("https://custom.example.com/v1/embeddings")
    end

    it "returns the standard endpoint for a URL that does not end in /chat/completions" do
      adapter = described_class.new(api_key: "k",
                                    endpoint_url: "https://weird.example.com/api")
      expect(adapter.send(:embeddings_endpoint)).to eq("https://api.openai.com/v1/embeddings")
    end
  end
end
