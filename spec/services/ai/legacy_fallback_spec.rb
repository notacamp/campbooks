require "rails_helper"

RSpec.describe Ai::LegacyFallback do
  it "disabled on the managed cloud even when ANTHROPIC_API_KEY is present" do
    with_env("ANTHROPIC_API_KEY" => "sk-test") do
      expect(described_class.allowed?).to be_falsey
    end
  end

  it "enabled on self-hosted when the operator set their own ANTHROPIC_API_KEY" do
    with_self_hosted do
      with_env("ANTHROPIC_API_KEY" => "sk-test") do
        expect(described_class.allowed?).to be_truthy
      end
    end
  end

  it "disabled on self-hosted without an ANTHROPIC_API_KEY" do
    with_self_hosted do
      with_env("ANTHROPIC_API_KEY" => nil) do
        expect(described_class.allowed?).to be_falsey
      end
    end
  end

  # Representative wiring check: a service's legacy path fails closed (returns nil,
  # the "no AI configured" shape) on the cloud instead of calling Anthropic.
  it "a service legacy path returns nil on the cloud rather than calling Anthropic" do
    with_env("ANTHROPIC_API_KEY" => "sk-test") do
      expect(Tools::DraftReply.send(:call_legacy, "system", "user")).to be_nil
    end
  end
end
