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
end
