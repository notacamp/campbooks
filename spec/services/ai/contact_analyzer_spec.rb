require "rails_helper"

RSpec.describe Ai::ContactAnalyzer, type: :service do
  let(:contact) { create(:contact, :with_emails, messages_count: 5) }

  describe "#analyze!" do
    let(:claude_client) { instance_double(Anthropic::Client) }
    let(:fake_response) do
      double("messages",
        content: [
          double("content_block", type: "text", text: {
            name: "John Doe",
            organization: "Acme Corp",
            relationship_type: "vendor",
            context_summary: "John is the accounts payable contact at Acme Corp.",
            communication_patterns: {
              typical_topics: [ "invoicing", "payment" ],
              tone: "formal",
              urgency_level: "medium",
              primary_role: "accounts payable"
            }
          }.to_json)
        ]
      )
    end

    before do
      # The legacy single-provider Anthropic path is gated to self-hosted now
      # (Ai::LegacyFallback) so the managed cloud can't silently process contact
      # data on a shared key. These specs exercise the model call + parse/apply
      # logic, so allow the fallback here regardless of the residency gate.
      allow(Ai::LegacyFallback).to receive(:allowed?).and_return(true)
      allow(Anthropic::Client).to receive(:new).and_return(claude_client)
      allow(claude_client).to receive_message_chain(:messages, :create).and_return(fake_response)
    end

    it "updates contact with AI analysis results" do
      described_class.new(contact).analyze!

      contact.reload
      expect(contact.name).to eq("John Doe")
      expect(contact.organization).to eq("Acme Corp")
      expect(contact.relationship_type).to eq("vendor")
      expect(contact.context_summary).to eq("John is the accounts payable contact at Acme Corp.")
      expect(contact.communication_patterns).to include("tone" => "formal")
      expect(contact.analyzed_at).to be_present
      expect(contact.raw_analysis).to be_present
    end

    it "skips if analyzed recently and not forced" do
      # Analysis is person-centric, so recency is tracked on the linked Person.
      person = create(:person, workspace: contact.workspace, analyzed_at: 5.days.ago)
      contact.update!(person: person, analyzed_at: 5.days.ago)
      described_class.new(contact).analyze!
      expect(claude_client).not_to have_received(:messages)
    end

    it "re-analyzes when forced" do
      person = create(:person, workspace: contact.workspace, analyzed_at: 5.days.ago)
      contact.update!(person: person, analyzed_at: 5.days.ago)
      described_class.new(contact).analyze!(force: true)
      expect(claude_client).to have_received(:messages)
    end

    it "skips if contact has no emails" do
      empty_contact = create(:contact)
      described_class.new(empty_contact).analyze!
      expect(claude_client).not_to have_received(:messages)
    end

    it "handles API errors gracefully" do
      allow(claude_client).to receive_message_chain(:messages, :create).and_raise(StandardError.new("API error"))
      expect {
        described_class.new(contact).analyze!
      }.not_to raise_error
    end

    it "handles invalid JSON response" do
      bad_response = double("messages",
        content: [ double("content_block", type: "text", text: "not json") ]
      )
      allow(claude_client).to receive_message_chain(:messages, :create).and_return(bad_response)

      expect {
        described_class.new(contact).analyze!
      }.not_to raise_error
    end
  end
end
