require "test_helper"

module Ai
  class PlatformTest < ActiveSupport::TestCase
    # Resolve the env keys from the managed providers so these tests don't care
    # whether managed text is Mistral, DeepSeek, etc.
    TEXT_KEY = AiAdapter::PROVIDER_ENV_KEYS[Ai::Platform::MANAGED_TEXT_PROVIDER]
    DOC_KEY  = AiAdapter::PROVIDER_ENV_KEYS[Ai::Platform::MANAGED_DOC_PROVIDER]

    test "available? is false on self-hosted even with the platform key" do
      with_self_hosted do
        with_env(TEXT_KEY => "k") { assert_not Ai::Platform.available? }
      end
    end

    test "available? is false when the text provider key is missing" do
      with_env(TEXT_KEY => nil) { assert_not Ai::Platform.available? }
    end

    test "available? is true on cloud with the text provider key" do
      with_env(TEXT_KEY => "k") { assert Ai::Platform.available? }
    end

    test "documents_available? additionally requires the document provider key" do
      with_env(TEXT_KEY => "k", DOC_KEY => nil) do
        assert Ai::Platform.available?
        assert_not Ai::Platform.documents_available?
      end
      with_env(TEXT_KEY => "k", DOC_KEY => "k2") do
        assert Ai::Platform.documents_available?
      end
    end

    test "managed models mirror the BYO defaults for their providers" do
      assert_equal AiConfiguration::DEFAULT_MODEL[Ai::Platform::MANAGED_TEXT_PROVIDER], Ai::Platform.text_model
      assert_equal AiConfiguration::DEFAULT_MODEL[Ai::Platform::MANAGED_DOC_PROVIDER], Ai::Platform.doc_model
      assert Ai::Platform.text_model.present?
    end
  end
end
