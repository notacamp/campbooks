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
  end
end
