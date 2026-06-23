require "rails_helper"

RSpec.describe ContactAnalysisJob, type: :job do
  let(:contact) { create(:contact, :with_emails, messages_count: 5) }

  describe "#perform" do
    let(:analyzer) { instance_double(Ai::ContactAnalyzer) }

    before do
      # The workspace has a text provider set up (otherwise analysis is skipped).
      allow(Ai::ProviderSetup).to receive(:configured?).and_return(true)
      allow(Ai::ContactAnalyzer).to receive(:new).with(contact, user_prompt: nil).and_return(analyzer)
      allow(analyzer).to receive(:analyze!)
    end

    it "calls ContactAnalyzer on the contact" do
      described_class.perform_now(contact.id)

      expect(Ai::ContactAnalyzer).to have_received(:new).with(contact, user_prompt: nil)
      expect(analyzer).to have_received(:analyze!).with(force: false)
    end

    it "skips recently analyzed contacts" do
      contact.update!(analyzed_at: 5.days.ago)

      described_class.perform_now(contact.id)
      expect(analyzer).not_to have_received(:analyze!)
    end

    it "re-analyzes when forced" do
      contact.update!(analyzed_at: 5.days.ago)

      described_class.perform_now(contact.id, force: true)
      expect(analyzer).to have_received(:analyze!).with(force: true)
    end

    it "handles missing contact gracefully" do
      expect {
        described_class.perform_now(-1)
      }.not_to raise_error
    end

    it "skips analysis when no text AI provider is configured" do
      allow(Ai::ProviderSetup).to receive(:configured?).and_return(false)

      described_class.perform_now(contact.id)
      expect(Ai::ContactAnalyzer).not_to have_received(:new)
    end
  end
end
