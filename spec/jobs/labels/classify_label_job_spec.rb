require "rails_helper"

RSpec.describe Labels::ClassifyLabelJob, type: :job do
  let(:workspace) { create(:workspace) }
  let(:account)   { create(:email_account, workspace: workspace, provider: :google) }

  def external_tag(attrs = {})
    Tag.create!({ workspace: workspace, email_account: account, source: :external,
                  external_label_id: "L#{SecureRandom.hex(3)}", name: "Invoices",
                  color: "#ccc" }.merge(attrs))
  end

  describe ".classify (sync-time dispatch)" do
    before { allow(described_class).to receive(:perform_later) }

    it "resolves a deterministic system label inline, without enqueuing" do
      tag = external_tag(external_label_id: "INBOX", name: "Inbox")
      described_class.classify(tag)

      expect(tag.reload).to be_hidden
      expect(tag.kind_system?).to be(true)
      expect(tag.classified_at).to be_present
      expect(described_class).not_to have_received(:perform_later)
    end

    it "enqueues an ambiguous label for async AI classification" do
      tag = external_tag(external_label_id: "Label_42", name: "Invoices")
      described_class.classify(tag)

      expect(tag.reload.classified_at).to be_nil
      expect(described_class).to have_received(:perform_later).with(tag.id).once
    end

    it "is idempotent for an already-classified tag" do
      tag = external_tag(classified_at: Time.current)
      described_class.classify(tag)
      expect(described_class).not_to have_received(:perform_later)
    end
  end

  describe "#perform" do
    it "keeps the label visible when no text provider is configured" do
      allow(Ai::ProviderSetup).to receive(:configured?).with(workspace, :text).and_return(false)
      tag = external_tag(external_label_id: "Label_99", name: "Invoices")

      described_class.new.perform(tag.id)

      tag.reload
      expect(tag.hidden?).to be(false)
      expect(tag.kind_user?).to be(true)
      expect(tag.classified_at).to be_present
    end

    it "hides a low_value AI verdict and deletes its message assignments" do
      allow(Ai::ProviderSetup).to receive(:configured?).with(workspace, :text).and_return(true)
      tag = external_tag(external_label_id: "Label_77", name: "Mailing Lists")
      message = create(:email_message, email_account: account)
      EmailMessageTag.create!(email_message: message, tag: tag)

      classifier = instance_double(Labels::AiClassifier,
                                   classify: { kind: :low_value, hidden: true, confidence: 0.9, reason: "noise" })
      allow(Labels::AiClassifier).to receive(:new).and_return(classifier)

      described_class.new.perform(tag.id)

      expect(tag.reload.hidden?).to be(true)
      expect(EmailMessageTag.where(tag_id: tag.id)).to be_empty
    end

    it "falls open (keeps visible) when the AI classifier returns nil" do
      allow(Ai::ProviderSetup).to receive(:configured?).with(workspace, :text).and_return(true)
      tag = external_tag(external_label_id: "Label_88", name: "Hmm")
      allow(Labels::AiClassifier).to receive(:new).and_return(instance_double(Labels::AiClassifier, classify: nil))

      described_class.new.perform(tag.id)

      expect(tag.reload.hidden?).to be(false)
    end

    it "no-ops for an already-classified tag" do
      tag = external_tag(classified_at: 1.hour.ago, hidden: false)
      expect(Ai::ProviderSetup).not_to receive(:configured?)
      described_class.new.perform(tag.id)
    end
  end
end
