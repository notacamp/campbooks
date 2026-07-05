require "rails_helper"

RSpec.describe AiAdapter do
  before do
    @ws = Workspace.create!(name: "AiAdapter Test WS")
  end

  it "managed defaults to false" do
    adapter = @ws.ai_adapters.create!(name: "byo", provider: "deepseek", api_key: "k")
    expect(adapter).not_to be_managed
  end

  it "api_key_source is stored when a key is present" do
    adapter = @ws.ai_adapters.create!(name: "byo", provider: "deepseek", api_key: "k")
    expect(adapter.api_key_source).to eq("stored")
  end

  it "api_key_source is managed for a managed adapter" do
    adapter = @ws.ai_adapters.create!(name: "managed", provider: "deepseek", managed: true)
    expect(adapter.api_key_source).to eq("managed")
  end

  it "api_key_source is missing when no key, not managed, not self-hosted" do
    adapter = @ws.ai_adapters.create!(name: "bare", provider: "deepseek")
    expect(adapter.api_key_source).to eq("missing")
  end

  it "managed adapter is usable only when the platform env key is present" do
    adapter = @ws.ai_adapters.create!(name: "managed", provider: "deepseek", managed: true)
    with_env("DEEPSEEK_API_KEY" => "platform-secret") { expect(adapter).to be_usable }
    with_env("DEEPSEEK_API_KEY" => nil) { expect(adapter).not_to be_usable }
  end

  it "managed adapter resolves the platform env key in adapter_instance" do
    adapter = @ws.ai_adapters.create!(name: "managed", provider: "deepseek", managed: true)
    with_env("DEEPSEEK_API_KEY" => "platform-secret") do
      expect { adapter.adapter_instance }.not_to raise_error
    end
  end

  it "byo adapter with a stored key is usable" do
    adapter = @ws.ai_adapters.create!(name: "byo", provider: "openai", api_key: "k")
    expect(adapter).to be_usable
  end

  it "managed adapter may not store an api_key" do
    adapter = @ws.ai_adapters.new(name: "bad", provider: "deepseek", managed: true, api_key: "leak")
    expect(adapter).not_to be_valid
    expect(adapter.errors[:api_key]).to be_any
  end

  it "managed adapter is invalid on a self-hosted install" do
    with_self_hosted do
      adapter = @ws.ai_adapters.new(name: "managed", provider: "deepseek", managed: true)
      expect(adapter).not_to be_valid
      expect(adapter.errors[:managed]).to be_any
    end
  end

  it "self-hosted byo adapter falls back to the operator env key" do
    adapter = @ws.ai_adapters.create!(name: "byo", provider: "deepseek")
    with_self_hosted do
      with_env("DEEPSEEK_API_KEY" => "operator-key") do
        expect(adapter).to be_usable
        expect(adapter.api_key_source).to eq("env")
      end
    end
  end

  it "rejects an endpoint_url aimed at the cloud metadata endpoint (SSRF)" do
    adapter = @ws.ai_adapters.new(name: "ssrf", provider: "openai", api_key: "k",
                                  endpoint_url: "http://169.254.169.254/latest/meta-data/")
    expect(adapter).not_to be_valid
    expect(adapter.errors[:endpoint_url]).to be_any
  end

  it "rejects an endpoint_url aimed at loopback with a trailing dot (SSRF)" do
    adapter = @ws.ai_adapters.new(name: "ssrf2", provider: "openai", api_key: "k",
                                  endpoint_url: "http://127.0.0.1./v1")
    expect(adapter).not_to be_valid
    expect(adapter.errors[:endpoint_url]).to be_any
  end

  it "allows a public https endpoint_url" do
    # Use a public literal IP so the guard takes the no-DNS path (hermetic).
    adapter = @ws.ai_adapters.new(name: "byo-endpoint", provider: "openai", api_key: "k",
                                  endpoint_url: "https://93.184.216.34/v1")
    expect(adapter).to be_valid, adapter.errors.full_messages.to_sentence
  end
end
