require "test_helper"

module Ai
  class PlatformTest < ActiveSupport::TestCase
    TEXT_KEY = AiAdapter::PROVIDER_ENV_KEYS[Ai::Platform::MANAGED_TEXT_PROVIDER]

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

    test "documents_available? mirrors available? — both use the same Mistral key" do
      with_env(TEXT_KEY => "k") do
        assert Ai::Platform.available?
        assert Ai::Platform.documents_available?
      end
      with_env(TEXT_KEY => nil) do
        assert_not Ai::Platform.available?
        assert_not Ai::Platform.documents_available?
      end
    end

    test "managed models use the right defaults — text vs doc" do
      assert_equal AiConfiguration::DEFAULT_MODEL[Ai::Platform::MANAGED_TEXT_PROVIDER], Ai::Platform.text_model
      assert_equal AiConfiguration::DOC_DEFAULT_MODEL[Ai::Platform::MANAGED_DOC_PROVIDER], Ai::Platform.doc_model
      assert Ai::Platform.text_model.present?
      assert Ai::Platform.doc_model.present?
    end
  end
end
