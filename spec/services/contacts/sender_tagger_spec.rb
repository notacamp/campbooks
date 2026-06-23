require "rails_helper"

RSpec.describe Contacts::SenderTagger do
  let(:workspace) { create(:workspace) }
  let(:contact) { create(:contact, :analyzed, workspace: workspace) }
  let!(:invoices)    { workspace.tags.create!(name: "invoices", color: "#111111") }
  let!(:newsletters) { workspace.tags.create!(name: "newsletters", color: "#222222") }

  describe ".parse_choices" do
    it "keeps in-range, deduped, capped indices and treats 0/blank as none" do
      expect(described_class.parse_choices("2, 5 and 1", 6)).to eq([ 2, 5, 1 ])
      expect(described_class.parse_choices("0", 6)).to eq([])
      expect(described_class.parse_choices("", 6)).to eq([])
      expect(described_class.parse_choices("7,7,1", 6)).to eq([ 1 ]) # 7 out of range, deduped
      expect(described_class.parse_choices("1 2 3 4", 6)).to eq([ 1, 2, 3 ]) # capped at MAX_TAGS
    end
  end

  describe "#call" do
    it "assigns existing workspace tags as auto contact_tags and stamps auto_tagged_at" do
      chosen = described_class.new(contact, completion: ->(_c, _tags) { "1, 2" }).call

      expect(chosen.map(&:name)).to match_array(%w[invoices newsletters])
      expect(contact.reload.sender_tags.pluck(:name)).to match_array(%w[invoices newsletters])
      expect(contact.contact_tags).to all(be_auto)
      expect(contact.auto_tagged_at).to be_present
    end

    it "never creates new tags — returns [] when the workspace has none" do
      Tag.where(workspace: workspace).destroy_all
      expect(described_class.new(contact, completion: ->(*) { "1" }).call).to eq([])
    end

    it "is idempotent (no duplicate contact_tags on repeat)" do
      tagger = described_class.new(contact, completion: ->(*) { "1" })
      tagger.call
      expect { tagger.call }.not_to change { contact.reload.contact_tags.count }
    end
  end
end
