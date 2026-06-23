require "rails_helper"

RSpec.describe Ai::ProviderSetup do
  # Managed adapters are invalid on self-hosted; force cloud mode so fixtures build.
  before { allow(Rails.application.config).to receive(:self_hosted).and_return(false) }

  describe ".apply_managed_default" do
    it "puts a brand-new workspace on managed text + document AI when the platform is available" do
      allow(Ai::Platform).to receive(:available?).and_return(true)
      allow(Ai::Platform).to receive(:documents_available?).and_return(true)
      ws = create(:workspace)

      result = described_class.apply_managed_default(ws)

      expect(result).to be(true)
      setup = described_class.new(ws)
      expect(setup.using_managed?).to be(true)
      expect(setup.text_provider).to eq(Ai::Platform::MANAGED_TEXT_PROVIDER)
      expect(setup.document_provider).to eq(Ai::Platform::MANAGED_DOC_PROVIDER)
      expect(setup.available?(:documents)).to be(true)
    end

    it "applies managed text even when document AI isn't available yet (no OpenAI key)" do
      allow(Ai::Platform).to receive(:available?).and_return(true)
      allow(Ai::Platform).to receive(:documents_available?).and_return(false)
      ws = create(:workspace)

      expect(described_class.apply_managed_default(ws)).to be(true)
      setup = described_class.new(ws)
      expect(setup.using_managed?).to be(true)
      expect(setup.available?(:documents)).to be(false)
    end

    it "is a no-op when the platform isn't available (self-hosted / no platform key)" do
      allow(Ai::Platform).to receive(:available?).and_return(false)
      ws = create(:workspace)

      expect(described_class.apply_managed_default(ws)).to be(false)
      expect(described_class.new(ws).using_managed?).to be(false)
    end

    it "returns false for a nil workspace without touching the platform" do
      expect(described_class.apply_managed_default(nil)).to be(false)
    end

    it "never raises — a provisioning failure is swallowed (best-effort)" do
      allow(Ai::Platform).to receive(:available?).and_return(true)
      ws = create(:workspace)
      allow_any_instance_of(described_class).to receive(:apply_managed).and_raise(StandardError, "boom")

      expect(described_class.apply_managed_default(ws)).to be(false)
    end
  end
end
