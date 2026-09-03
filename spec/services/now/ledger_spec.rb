require "rails_helper"

# Now::Ledger is Scout's honest opening tally: it only ever counts what the
# workspace Event log proves (docs/messaging.md), reading SYSTEM events
# (actor_id IS NULL) accessible to the user within the window and bucketing them
# by event name.
RSpec.describe Now::Ledger do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }

  # A workspace SYSTEM event (no actor = Scout/automation), inside the window.
  def sys(name, occurred_at: 1.hour.ago, payload: {}, actor: nil)
    create(:event, workspace: workspace, name: name, actor: actor, occurred_at: occurred_at, payload: payload)
  end

  describe "#buckets" do
    it "buckets events by name, largest first" do
      3.times { sys("email.archived") }
      5.times { sys("email.tagged", payload: { "tag" => "Receipts" }) }
      2.times { sys("document.processed") }

      ledger = described_class.new(user)

      expect(ledger.buckets).to eq([
        { key: :tagged, count: 5 },
        { key: :archived, count: 3 },
        { key: :filed, count: 2 }
      ])
    end

    it "counts email.bulk_archived by its payload count, into the archived bucket" do
      sys("email.bulk_archived", payload: { "count" => 24 })

      expect(described_class.new(user).buckets).to eq([ { key: :archived, count: 24 } ])
    end

    it "caps at the top three buckets" do
      4.times { sys("email.archived") }
      3.times { sys("email.tagged") }
      2.times { sys("document.processed") }
      1.times { sys("reminder.created") }

      buckets = described_class.new(user).buckets
      expect(buckets.size).to eq(3)
      expect(buckets.map { |b| b[:key] }).to eq(%i[archived tagged filed])
    end

    it "only counts AI-suggested tasks (a manual task is not Scout's doing)" do
      sys("task.created", payload: { "ai_suggested" => true })
      sys("task.created", payload: { "ai_suggested" => false })

      expect(described_class.new(user).buckets).to eq([ { key: :tasks, count: 1 } ])
    end

    it "ignores events with a human actor (only Scout/system events count)" do
      sys("email.archived")
      sys("email.archived", actor: user) # a person did this — not Scout

      expect(described_class.new(user).buckets).to eq([ { key: :archived, count: 1 } ])
    end

    it "ignores events outside the 24-hour window" do
      sys("email.archived", occurred_at: 2.hours.ago)
      sys("email.archived", occurred_at: 30.hours.ago)

      expect(described_class.new(user).buckets).to eq([ { key: :archived, count: 1 } ])
    end

    it "is empty (and #any? false) when nothing happened" do
      ledger = described_class.new(user)
      expect(ledger.buckets).to eq([])
      expect(ledger.any?).to be(false)
    end
  end

  describe "#archived_by_rules?" do
    it "is true only when every archived event proves a rule drove it" do
      sys("email.archived", payload: { "rule_name" => "Newsletters" })
      sys("email.archived", payload: { "rule_id" => 7 })

      expect(described_class.new(user).archived_by_rules?).to be(true)
    end

    it "is false when even one archive has no rule attribution" do
      sys("email.archived", payload: { "rule_name" => "Newsletters" })
      sys("email.archived") # no rule marker

      expect(described_class.new(user).archived_by_rules?).to be(false)
    end

    it "is false when there are no archives at all" do
      sys("email.tagged")
      expect(described_class.new(user).archived_by_rules?).to be(false)
    end
  end

  describe "#total and #need_you" do
    it "totals every counted action across all buckets" do
      3.times { sys("email.archived") }
      2.times { sys("email.tagged") }

      expect(described_class.new(user).total).to eq(5)
    end

    it "passes need_you straight through" do
      expect(described_class.new(user, need_you: 4).need_you).to eq(4)
    end
  end
end
