require "rails_helper"

RSpec.describe Campbooks::Base, type: :component do
  describe ".class_names" do
    it "resolves conflicting utilities so the last one wins (override semantics)" do
      expect(described_class.class_names("rounded-lg", "rounded-none")).to eq("rounded-none")
      expect(described_class.class_names("block", "flex")).to eq("flex")
    end

    it "filters out nil / false / blank tokens" do
      expect(described_class.class_names("px-2", nil, false, "", "py-1")).to eq("px-2 py-1")
      expect(described_class.class_names).to eq("")
    end

    # Regression: a bare `hidden` must survive merging with a layout display
    # utility, otherwise "hidden by default, shown on toggle" markup renders
    # visible. `.hidden` wins in the CSS anyway, so re-appending it is faithful.
    it "preserves a bare `hidden` even when a layout display utility follows" do
      expect(described_class.class_names("hidden", "flex")).to include("hidden")
      expect(described_class.class_names("hidden", "fixed inset-0 flex")).to include("hidden")
    end

    it "keeps `hidden` when combined with non-display utilities (unchanged)" do
      expect(described_class.class_names("px-3 pb-3", "hidden")).to eq("px-3 pb-3 hidden")
    end

    it "does not duplicate `hidden` or disturb responsive visibility variants" do
      expect(described_class.class_names("hidden sm:block")).to eq("hidden sm:block")
      expect(described_class.class_names("flex", "hidden")).to eq("hidden")
    end
  end
end
