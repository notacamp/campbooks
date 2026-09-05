# frozen_string_literal: true

require "rails_helper"

RSpec.describe EmailAnalysisJob do
  let(:workspace) { create(:workspace) }
  let(:account)   { create(:email_account, workspace: workspace) }
  let(:contact)   { create(:contact, workspace: workspace, email_account: account,
                            email: "sender@x.example", sender_kind: :person) }
  let(:email)     { create(:email_message, email_account: account, contact: contact,
                            received_at: 1.day.ago) }

  before do
    # Stub the analyzer so we don't need real AI in specs.
    allow(Ai::EmailAnalyzer).to receive(:new).and_return(double(analyze!: true))
    allow(Feed::RefreshJob).to receive(:enqueue_for_account)
  end

  describe "#perform" do
    context "when AI text is configured" do
      before do
        allow(Ai::ProviderSetup).to receive(:configured?).with(workspace, :text).and_return(true)
      end

      it "calls EmailAnalyzer#analyze! and enqueues Feed::RefreshJob" do
        described_class.new.perform(email.id)
        expect(Ai::EmailAnalyzer).to have_received(:new).with(email)
        expect(Feed::RefreshJob).to have_received(:enqueue_for_account).with(account)
      end

      it "skips already-analyzed emails" do
        email.update_columns(ai_analyzed_at: 1.hour.ago)
        described_class.new.perform(email.id)
        expect(Ai::EmailAnalyzer).not_to have_received(:new)
      end

      it "skips security_flagged emails" do
        tag = create(:tag, workspace: workspace, name: "security_flagged")
        email.email_message_tags.create!(tag: tag)
        described_class.new.perform(email.id)
        expect(Ai::EmailAnalyzer).not_to have_received(:new)
      end
    end

    context "when AI text is not configured" do
      before do
        allow(Ai::ProviderSetup).to receive(:configured?).with(workspace, :text).and_return(false)
      end

      it "skips analysis" do
        described_class.new.perform(email.id)
        expect(Ai::EmailAnalyzer).not_to have_received(:new)
      end
    end

    it "warns and returns silently when the email record is missing" do
      expect(Rails.logger).to receive(:warn).with(/not found/)
      expect { described_class.new.perform(SecureRandom.uuid) }.not_to raise_error
    end

    it "clears Current.workspace in the ensure block even on error" do
      allow(Ai::ProviderSetup).to receive(:configured?).and_return(true)
      allow(Ai::EmailAnalyzer).to receive(:new).and_raise(RuntimeError, "boom")

      expect { described_class.new.perform(email.id) }.to raise_error(RuntimeError)
      expect(Current.workspace).to be_nil
    end
  end
end
