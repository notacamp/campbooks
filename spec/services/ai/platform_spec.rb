require "rails_helper"

RSpec.describe Ai::Platform do
  let(:text_key) { AiAdapter::PROVIDER_ENV_KEYS[Ai::Platform::MANAGED_TEXT_PROVIDER] }

  it "available? is false on self-hosted even with the platform key" do
    with_self_hosted do
      with_env(text_key => "k") { expect(described_class.available?).to be_falsey }
    end
  end

  it "available? is false when the text provider key is missing" do
    with_env(text_key => nil) { expect(described_class.available?).to be_falsey }
  end

  it "available? is true on cloud with the text provider key" do
    with_env(text_key => "k") { expect(described_class.available?).to be_truthy }
  end

  it "documents_available? mirrors available? — both use the same Mistral key" do
    with_env(text_key => "k") do
      expect(described_class.available?).to be_truthy
      expect(described_class.documents_available?).to be_truthy
    end
    with_env(text_key => nil) do
      expect(described_class.available?).to be_falsey
      expect(described_class.documents_available?).to be_falsey
    end
  end

  it "managed models use the right defaults — text vs doc" do
    expect(described_class.text_model).to eq(AiConfiguration::DEFAULT_MODEL[Ai::Platform::MANAGED_TEXT_PROVIDER])
    expect(described_class.doc_model).to eq(AiConfiguration::DOC_DEFAULT_MODEL[Ai::Platform::MANAGED_DOC_PROVIDER])
    expect(described_class.text_model).to be_present
    expect(described_class.doc_model).to be_present
  end
end
