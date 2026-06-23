require "test_helper"

class AiAdapterTest < ActiveSupport::TestCase
  setup do
    @ws = Workspace.create!(name: "AiAdapter Test WS")
  end

  test "managed defaults to false" do
    adapter = @ws.ai_adapters.create!(name: "byo", provider: "deepseek", api_key: "k")
    assert_not adapter.managed?
  end

  test "api_key_source is stored when a key is present" do
    adapter = @ws.ai_adapters.create!(name: "byo", provider: "deepseek", api_key: "k")
    assert_equal "stored", adapter.api_key_source
  end

  test "api_key_source is managed for a managed adapter" do
    adapter = @ws.ai_adapters.create!(name: "managed", provider: "deepseek", managed: true)
    assert_equal "managed", adapter.api_key_source
  end

  test "api_key_source is missing when no key, not managed, not self-hosted" do
    adapter = @ws.ai_adapters.create!(name: "bare", provider: "deepseek")
    assert_equal "missing", adapter.api_key_source
  end

  test "managed adapter is usable only when the platform env key is present" do
    adapter = @ws.ai_adapters.create!(name: "managed", provider: "deepseek", managed: true)
    with_env("DEEPSEEK_API_KEY" => "platform-secret") { assert adapter.usable? }
    with_env("DEEPSEEK_API_KEY" => nil) { assert_not adapter.usable? }
  end

  test "managed adapter resolves the platform env key in adapter_instance" do
    adapter = @ws.ai_adapters.create!(name: "managed", provider: "deepseek", managed: true)
    with_env("DEEPSEEK_API_KEY" => "platform-secret") do
      assert_nothing_raised { adapter.adapter_instance }
    end
  end

  test "byo adapter with a stored key is usable" do
    adapter = @ws.ai_adapters.create!(name: "byo", provider: "openai", api_key: "k")
    assert adapter.usable?
  end

  test "managed adapter may not store an api_key" do
    adapter = @ws.ai_adapters.new(name: "bad", provider: "deepseek", managed: true, api_key: "leak")
    assert_not adapter.valid?
    assert adapter.errors[:api_key].any?
  end

  test "managed adapter is invalid on a self-hosted install" do
    with_self_hosted do
      adapter = @ws.ai_adapters.new(name: "managed", provider: "deepseek", managed: true)
      assert_not adapter.valid?
      assert adapter.errors[:managed].any?
    end
  end

  test "self-hosted byo adapter falls back to the operator env key" do
    adapter = @ws.ai_adapters.create!(name: "byo", provider: "deepseek")
    with_self_hosted do
      with_env("DEEPSEEK_API_KEY" => "operator-key") do
        assert adapter.usable?
        assert_equal "env", adapter.api_key_source
      end
    end
  end

  test "rejects an endpoint_url aimed at the cloud metadata endpoint (SSRF)" do
    adapter = @ws.ai_adapters.new(name: "ssrf", provider: "openai", api_key: "k",
                                  endpoint_url: "http://169.254.169.254/latest/meta-data/")
    assert_not adapter.valid?
    assert adapter.errors[:endpoint_url].any?
  end

  test "rejects an endpoint_url aimed at loopback with a trailing dot (SSRF)" do
    adapter = @ws.ai_adapters.new(name: "ssrf2", provider: "openai", api_key: "k",
                                  endpoint_url: "http://127.0.0.1./v1")
    assert_not adapter.valid?
    assert adapter.errors[:endpoint_url].any?
  end

  test "allows a public https endpoint_url" do
    # Use a public literal IP so the guard takes the no-DNS path (hermetic).
    adapter = @ws.ai_adapters.new(name: "byo-endpoint", provider: "openai", api_key: "k",
                                  endpoint_url: "https://93.184.216.34/v1")
    assert adapter.valid?, adapter.errors.full_messages.to_sentence
  end
end
