require "test_helper"

module Ai
  class ProviderSetupTest < ActiveSupport::TestCase
    # Env keys for whatever providers currently back managed text/documents, so
    # these tests don't hardcode a specific provider default.
    TEXT_KEY = AiAdapter::PROVIDER_ENV_KEYS[Ai::Platform::MANAGED_TEXT_PROVIDER]
    DOC_KEY  = AiAdapter::PROVIDER_ENV_KEYS[Ai::Platform::MANAGED_DOC_PROVIDER]

    setup do
      @ws = Workspace.create!(name: "ProviderSetup Test WS")
      @setup = Ai::ProviderSetup.new(@ws)
    end

    test "apply_managed seeds a keyless managed text adapter and wires every text purpose" do
      with_env(TEXT_KEY => "k", DOC_KEY => "k2") do
        @setup.apply_managed

        text = @ws.ai_adapters.find_by(managed: true, provider: Ai::Platform::MANAGED_TEXT_PROVIDER)
        assert text, "expected a managed deepseek adapter"
        assert_nil text.api_key
        assert_empty AiConfiguration::TEXT_PURPOSES - @ws.ai_configurations.pluck(:purpose)
        assert @setup.using_managed?
        assert @setup.text_configured?
      end
    end

    test "apply_managed seeds the document adapter when the doc key is present" do
      with_env(TEXT_KEY => "k", DOC_KEY => "k2") do
        @setup.apply_managed
        assert @ws.ai_adapters.exists?(managed: true, provider: Ai::Platform::MANAGED_DOC_PROVIDER)
        assert @setup.documents_configured?
      end
    end

    test "apply_managed skips the document adapter when the doc key is missing" do
      with_env(TEXT_KEY => "k", DOC_KEY => nil) do
        @setup.apply_managed
        assert_not @ws.ai_adapters.exists?(managed: true, provider: Ai::Platform::MANAGED_DOC_PROVIDER)
        assert @setup.text_configured?
        assert_not @setup.documents_configured?
      end
    end

    test "apply_managed is idempotent" do
      with_env(TEXT_KEY => "k", DOC_KEY => "k2") do
        @setup.apply_managed
        assert_no_difference -> { @ws.ai_adapters.count } do
          assert_no_difference -> { @ws.ai_configurations.count } do
            @setup.apply_managed
          end
        end
      end
    end

    test "apply_managed raises on a self-hosted install" do
      with_self_hosted do
        assert_raises(RuntimeError) { @setup.apply_managed }
      end
    end

    test "text_configured? becomes false if the managed platform key disappears" do
      with_env(TEXT_KEY => "k", DOC_KEY => "k2") { @setup.apply_managed }
      with_env(TEXT_KEY => nil) { assert_not @setup.text_configured? }
    end

    test "switching managed to BYO via apply_text lands on the dedicated row and keeps the key" do
      with_env(TEXT_KEY => "k", DOC_KEY => "k2") do
        @setup.apply_managed
        @setup.apply_text(provider: "openai", api_key: "byo-secret")

        assert_not @setup.using_managed?
        adapter = @ws.ai_configurations.find_by(purpose: "global_chat").ai_adapter
        assert_not adapter.managed?
        assert adapter.api_key.present?
        assert @setup.text_configured?
      end
    end

    test "apply_text alone is bring-your-own (not managed)" do
      @setup.apply_text(provider: "openai", api_key: "k")
      assert_not @setup.using_managed?
      assert @setup.text_configured?
    end

    # --- Data-residency: availability must not count silent shared platform keys ---

    test "text_available? ignores a bare platform ANTHROPIC_API_KEY on the cloud" do
      with_env("ANTHROPIC_API_KEY" => "sk-test") do
        assert_not @setup.text_available?,
          "a bare shared Anthropic key must not count as configured AI on the cloud"
      end
    end

    test "text_available? counts the operator's own env key on self-hosted" do
      with_self_hosted do
        with_env("ANTHROPIC_API_KEY" => "sk-test") do
          assert @setup.text_available?
        end
      end
    end

    test "embeddings_available? ignores bare platform OPENAI/GEMINI keys on the cloud" do
      with_env("OPENAI_API_KEY" => "sk", "GEMINI_API_KEY" => "g") do
        assert_not @setup.embeddings_available?
      end
    end

    test "embeddings_available? is true with a configured OpenAI adapter" do
      @ws.ai_adapters.create!(name: "Embeds", provider: "openai", api_key: "byo", enabled: true)
      assert @setup.embeddings_available?
    end

    test "embeddings_available? counts the operator env key on self-hosted" do
      with_self_hosted do
        with_env("OPENAI_API_KEY" => "sk", "GEMINI_API_KEY" => nil) do
          assert @setup.embeddings_available?
        end
      end
    end
  end
end
