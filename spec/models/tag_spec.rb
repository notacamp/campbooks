require "rails_helper"

RSpec.describe Tag, type: :model do
  let(:workspace) { create(:workspace) }

  describe "kind enum" do
    it "defines user/system/category/low_value" do
      expect(Tag.kinds).to eq("user" => 0, "system" => 1, "category" => 2, "low_value" => 3)
    end

    it "defaults to user" do
      expect(Tag.new.kind_user?).to be(true)
    end
  end

  describe "scopes" do
    let!(:visible_tag) do
      Tag.create!(workspace: workspace, name: "Invoices", color: "#ccc", source: :local)
    end
    let!(:hidden_tag) do
      Tag.create!(workspace: workspace, name: "Inbox", color: "#ccc", source: :local,
                  hidden: true, kind: :system)
    end

    it "visible returns only non-hidden tags" do
      expect(Tag.visible).to include(visible_tag)
      expect(Tag.visible).not_to include(hidden_tag)
    end

    it "hidden_labels returns only hidden tags" do
      expect(Tag.hidden_labels).to contain_exactly(hidden_tag)
    end

    it "visible_for returns only non-hidden tags" do
      expect(Tag.visible_for(workspace)).to include(visible_tag)
      expect(Tag.visible_for(workspace)).not_to include(hidden_tag)
    end
  end

  describe "#apply_classification!" do
    let(:tag) { Tag.create!(workspace: workspace, name: "Updates", color: "#ccc", source: :local) }

    it "persists the decision (kind/hidden/confidence/reason/classified_at)" do
      tag.apply_classification!(kind: :low_value, hidden: true, confidence: 0.9, reason: "noise")

      tag.reload
      expect(tag.kind_low_value?).to be(true)
      expect(tag.hidden?).to be(true)
      expect(tag.classification_confidence).to eq(0.9)
      expect(tag.classification_reason).to eq("noise")
      expect(tag.classified_at).to be_present
    end

    it "accepts a symbol kind and maps it to the right enum value" do
      tag.apply_classification!(kind: :system, hidden: true)
      expect(tag.reload.kind_system?).to be(true)
    end
  end

  describe ".palette_color_for" do
    it "is deterministic for the same seed" do
      expect(Tag.palette_color_for("Invoices")).to eq(Tag.palette_color_for("Invoices"))
    end

    it "always returns a palette hex, never the old gold default" do
      %w[Invoices Família flights Apps whatever].each do |seed|
        color = Tag.palette_color_for(seed)
        expect(Tag::PALETTE).to include(color)
        expect(color).not_to eq("#ffd700")
      end
    end

    it "spreads distinct seeds across more than one colour" do
      colors = %w[alpha bravo charlie delta echo foxtrot golf hotel].map { |s| Tag.palette_color_for(s) }
      expect(colors.uniq.size).to be > 1
    end

    it "falls back to the first palette colour for a blank/nil seed" do
      expect(Tag.palette_color_for("")).to eq(Tag::PALETTE.first)
      expect(Tag.palette_color_for(nil)).to eq(Tag::PALETTE.first)
    end
  end
end
