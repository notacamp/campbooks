require "rails_helper"

RSpec.describe Emails::SkimActionMemory do
  let(:user) { create(:user) }

  # Create a decision, optionally back-dating it past the timestamps Rails would set.
  def decide(action, at: nil, **attrs)
    decision = SkimDecision.create!(user: user, workspace: user.workspace, action: action, **attrs)
    decision.update_columns(created_at: at) if at
    decision
  end

  describe "#suggestion_for by sender contact" do
    let(:contact) { create(:contact, workspace: user.workspace) }

    it "suggests the dominant action once a clear majority emerges" do
      3.times { decide("archive", contact_id: contact.id) }
      decide("keep", contact_id: contact.id)

      suggestion = described_class.new(user).suggestion_for(contact_id: contact.id)

      expect(suggestion).to include(action: "archive", source: :contact, count: 3, total: 4)
    end

    it "is nil below the minimum number of examples" do
      2.times { decide("archive", contact_id: contact.id) }

      expect(described_class.new(user).suggestion_for(contact_id: contact.id)).to be_nil
    end

    it "is nil when no single action holds the majority" do
      2.times { decide("archive", contact_id: contact.id) }
      2.times { decide("keep", contact_id: contact.id) }

      expect(described_class.new(user).suggestion_for(contact_id: contact.id)).to be_nil
    end

    it "ignores decisions older than the window" do
      3.times { decide("archive", contact_id: contact.id, at: 100.days.ago) }

      expect(described_class.new(user).suggestion_for(contact_id: contact.id)).to be_nil
    end

    it "learns only from this user's own decisions" do
      other = create(:user)
      3.times { SkimDecision.create!(user: other, workspace: other.workspace, action: "archive", contact_id: contact.id) }

      expect(described_class.new(user).suggestion_for(contact_id: contact.id)).to be_nil
    end
  end

  describe "signature precedence (contact → domain → category)" do
    it "prefers the sender contact over domain and category" do
      contact = create(:contact, workspace: user.workspace)
      # Contact consistently kept; the same domain/category trend toward other verbs.
      3.times { decide("keep", contact_id: contact.id, sender_domain: "github.com", category: "notifications") }
      3.times { decide("archive", sender_domain: "github.com") }
      3.times { decide("promote", category: "notifications") }

      suggestion = described_class.new(user).suggestion_for(
        contact_id: contact.id, sender_domain: "github.com", category: "notifications"
      )

      expect(suggestion).to include(action: "keep", source: :contact)
    end

    it "falls back to domain (case-insensitively) when the contact has no history" do
      3.times { decide("archive", sender_domain: "github.com") }

      suggestion = described_class.new(user).suggestion_for(contact_id: 999_999, sender_domain: "GitHub.com")

      expect(suggestion).to include(action: "archive", source: :domain)
    end

    it "falls back to category when neither contact nor domain match" do
      3.times { decide("archive", category: "promotions") }

      suggestion = described_class.new(user).suggestion_for(sender_domain: "unseen.com", category: "promotions")

      expect(suggestion).to include(action: "archive", source: :category)
    end

    it "returns nil when nothing is known about the cluster" do
      expect(described_class.new(user).suggestion_for(contact_id: 1, sender_domain: "x.com", category: "updates")).to be_nil
    end
  end

  it "preloads decisions once and reuses the tally across lookups" do
    contact = create(:contact, workspace: user.workspace)
    3.times { decide("archive", contact_id: contact.id) }
    memory = described_class.new(user)

    expect(memory.suggestion_for(contact_id: contact.id)).to include(action: "archive")

    # Wipe the table: a memory that preloaded once still answers from its in-memory
    # snapshot, proving the deck doesn't re-query per card.
    SkimDecision.delete_all
    expect(memory.suggestion_for(contact_id: contact.id)).to include(action: "archive")
  end
end
