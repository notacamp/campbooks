require "rails_helper"

RSpec.describe EmbeddingService do
  let(:ws) { Workspace.create!(name: "Embedding Test WS") }

  it "env-key fallback is disabled on the managed cloud (no silent platform-key embedding)" do
    with_env("OPENAI_API_KEY" => "sk-test", "GEMINI_API_KEY" => nil) do
      expect(described_class.new(ws).send(:env_fallback_adapter)).to be_nil,
        "embedding must not silently fall back to the platform OpenAI key on the cloud"
    end
  end

  it "env-key fallback uses the operator's own key on self-hosted" do
    with_self_hosted do
      with_env("OPENAI_API_KEY" => "sk-test", "GEMINI_API_KEY" => nil) do
        adapter = described_class.new(ws).send(:env_fallback_adapter)
        expect(adapter).to be_an_instance_of(Ai::Adapters::Openai)
      end
    end
  end

  it "env-key fallback returns nil on self-hosted without a key" do
    with_self_hosted do
      with_env("OPENAI_API_KEY" => nil, "GEMINI_API_KEY" => nil) do
        expect(described_class.new(ws).send(:env_fallback_adapter)).to be_nil
      end
    end
  end

  it "embed_batch fails closed (nil) on the cloud with no configured adapter" do
    with_env("OPENAI_API_KEY" => "sk-test", "GEMINI_API_KEY" => nil) do
      # No workspace embedding adapter + not self-hosted => no silent platform-key fallback.
      expect(described_class.new(ws).embed_batch([ "hello" ])).to be_nil
    end
  end
end
