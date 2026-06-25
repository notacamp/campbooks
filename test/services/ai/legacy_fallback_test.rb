require "test_helper"

module Ai
  class LegacyFallbackTest < ActiveSupport::TestCase
    test "disabled on the managed cloud even when ANTHROPIC_API_KEY is present" do
      with_env("ANTHROPIC_API_KEY" => "sk-test") do
        assert_not Ai::LegacyFallback.allowed?,
          "the shared platform Anthropic key must not be used for content on the cloud"
      end
    end

    test "enabled on self-hosted when the operator set their own ANTHROPIC_API_KEY" do
      with_self_hosted do
        with_env("ANTHROPIC_API_KEY" => "sk-test") do
          assert Ai::LegacyFallback.allowed?
        end
      end
    end

    test "disabled on self-hosted without an ANTHROPIC_API_KEY" do
      with_self_hosted do
        with_env("ANTHROPIC_API_KEY" => nil) do
          assert_not Ai::LegacyFallback.allowed?
        end
      end
    end

    # Representative wiring check: a service's legacy path fails closed (returns nil,
    # the "no AI configured" shape) on the cloud instead of calling Anthropic.
    test "a service legacy path returns nil on the cloud rather than calling Anthropic" do
      with_env("ANTHROPIC_API_KEY" => "sk-test") do
        assert_nil Tools::DraftReply.send(:call_legacy, "system", "user")
      end
    end
  end
end
