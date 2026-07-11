# frozen_string_literal: true

require "rails_helper"

RSpec.describe Ai::Adapters::Mistral do
  it "is an OpenAI-compatible adapter pointed at Mistral's EU endpoint by default" do
    adapter = described_class.new(api_key: "k")
    expect(adapter).to be_a(Ai::Adapters::Openai)
    expect(adapter.instance_variable_get(:@endpoint_url)).to eq("https://api.mistral.ai/v1/chat/completions")
  end

  it "honours a custom endpoint override" do
    adapter = described_class.new(api_key: "k", endpoint_url: "https://eu.example/v1/chat")
    expect(adapter.instance_variable_get(:@endpoint_url)).to eq("https://eu.example/v1/chat")
  end

  it "POSTs an OpenAI-style chat body to the Mistral endpoint" do
    adapter = described_class.new(api_key: "k")
    conn = instance_double(Faraday::Connection)
    allow(adapter).to receive(:connection).and_return(conn)
    response = instance_double(Faraday::Response, body: { choices: [ { message: { content: "bonjour" } } ] }.to_json)
    expect(conn).to receive(:post).with("https://api.mistral.ai/v1/chat/completions").and_return(response)

    result = adapter.chat(system: "sys", messages: [ { role: "user", content: "hi" } ], model: "mistral-small-latest", max_tokens: 100)
    expect(result).to eq("bonjour")
  end

  describe "#embeddings_endpoint" do
    it "always returns the Mistral embeddings URL regardless of chat endpoint" do
      adapter = described_class.new(api_key: "k")
      expect(adapter.send(:embeddings_endpoint)).to eq("https://api.mistral.ai/v1/embeddings")
    end
  end

  describe "#embed" do
    let(:adapter) { described_class.new(api_key: "mkey") }
    let(:request) { double("request") }
    let(:response) do
      double("response", body: { "data" => [ { "index" => 0, "embedding" => Array.new(1024) { 0.01 } } ] }.to_json)
    end
    let(:connection) { double("connection") }

    before do
      allow(adapter).to receive(:connection).and_return(connection)
      allow(request).to receive(:body=)
      allow(connection).to receive(:post)
        .with("https://api.mistral.ai/v1/embeddings")
        .and_yield(request).and_return(response)
    end

    it "POSTs to the Mistral embeddings endpoint" do
      result = adapter.embed("hello", model: "mistral-embed")
      expect(result).to be_an(Array)
      expect(result.length).to eq(1024)
    end

    it "does not include dimensions in the request body (nil request_dimensions)" do
      adapter.embed("hello", model: "mistral-embed", dimensions: nil)
      expect(request).to have_received(:body=) do |json|
        expect(JSON.parse(json)).not_to have_key("dimensions")
      end
    end
  end
end
