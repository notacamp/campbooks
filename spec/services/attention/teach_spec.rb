# frozen_string_literal: true

require "rails_helper"

RSpec.describe Attention::Teach do
  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace) }
  let(:account)   { create(:email_account, workspace: workspace) }

  before do
    create(:email_account_user, user: user, email_account: account, can_read: true)
  end

  let(:person) { create(:person, workspace: workspace, name: "Sofia Martins") }
  let!(:contact) do
    create(:contact, workspace: workspace, email_account: account,
           person: person, email: "sofia@example.com",
           sender_kind: :person, email_count: 3)
  end

  describe ".record" do
    it "writes one LearningDecision row per contact for the person" do
      expect {
        described_class.record(person: person, user: user, label: "important")
      }.to change { LearningDecision.where(domain: "attention", user: user).count }.by(1)

      row = LearningDecision.where(domain: "attention", user: user).last
      expect(row.label).to eq("important")
      expect(row.contact_id).to eq(contact.id)
      expect(row.signals["source"]).to eq("rail")
    end

    it "writes one row per contact when a person has multiple contacts" do
      extra = create(:contact, workspace: workspace, email_account: account,
                     person: person, email: "sofia.m@example.com",
                     sender_kind: :person, email_count: 1)

      expect {
        described_class.record(person: person, user: user, label: "important", source: "memory")
      }.to change { LearningDecision.where(domain: "attention", user: user).count }.by(2)

      rows = LearningDecision.where(domain: "attention", user: user)
      expect(rows.all? { |r| r.signals["source"] == "memory" }).to be(true)
    end

    it "returns true" do
      result = described_class.record(person: person, user: user, label: "important")
      expect(result).to be(true)
    end

    it "writes unimportant label" do
      described_class.record(person: person, user: user, label: "unimportant", source: "teach")
      row = LearningDecision.where(domain: "attention", user: user).last
      expect(row.label).to eq("unimportant")
      expect(row.signals["source"]).to eq("teach")
    end
  end

  describe ".forget" do
    it "deletes all attention rows for the person" do
      described_class.record(person: person, user: user, label: "important")
      described_class.record(person: person, user: user, label: "unimportant")

      expect {
        described_class.forget(person: person, user: user)
      }.to change { LearningDecision.where(domain: "attention", user: user, contact_id: contact.id).count }.to(0)
    end

    it "does not delete another user's rows" do
      other_user = create(:user, workspace: workspace)
      described_class.record(person: person, user: other_user, label: "important")

      described_class.forget(person: person, user: user)
      expect(LearningDecision.where(domain: "attention", user: other_user).count).to eq(1)
    end

    it "returns true" do
      expect(described_class.forget(person: person, user: user)).to be(true)
    end
  end

  describe ".verdict" do
    it "returns nil when no rows exist" do
      expect(described_class.verdict(person: person, user: user)).to be_nil
    end

    it "returns the label of the newest row" do
      described_class.record(person: person, user: user, label: "important")
      described_class.record(person: person, user: user, label: "unimportant")

      expect(described_class.verdict(person: person, user: user)).to eq("unimportant")
    end

    it "returns nil after forget" do
      described_class.record(person: person, user: user, label: "important")
      described_class.forget(person: person, user: user)
      expect(described_class.verdict(person: person, user: user)).to be_nil
    end
  end
end
