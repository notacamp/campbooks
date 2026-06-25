require "test_helper"

class EmbeddingServiceTest < ActiveSupport::TestCase
  setup { @ws = Workspace.create!(name: "Embedding Test WS") }

  test "env-key fallback is disabled on the managed cloud (no silent platform-key embedding)" do
    with_env("OPENAI_API_KEY" => "sk-test", "GEMINI_API_KEY" => nil) do
      assert_nil EmbeddingService.new(@ws).send(:env_fallback_adapter),
        "embedding must not silently fall back to the platform OpenAI key on the cloud"
    end
  end

  test "env-key fallback uses the operator's own key on self-hosted" do
    with_self_hosted do
      with_env("OPENAI_API_KEY" => "sk-test", "GEMINI_API_KEY" => nil) do
        adapter = EmbeddingService.new(@ws).send(:env_fallback_adapter)
        assert_instance_of Ai::Adapters::Openai, adapter
      end
    end
  end

  test "env-key fallback returns nil on self-hosted without a key" do
    with_self_hosted do
      with_env("OPENAI_API_KEY" => nil, "GEMINI_API_KEY" => nil) do
        assert_nil EmbeddingService.new(@ws).send(:env_fallback_adapter)
      end
    end
  end

  test "embed_batch fails closed (nil) on the cloud with no configured adapter" do
    with_env("OPENAI_API_KEY" => "sk-test", "GEMINI_API_KEY" => nil) do
      # No workspace embedding adapter + not self-hosted ⇒ no silent platform-key fallback.
      assert_nil EmbeddingService.new(@ws).embed_batch([ "hello" ])
    end
  end
end
