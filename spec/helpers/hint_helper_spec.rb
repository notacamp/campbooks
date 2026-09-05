require "rails_helper"

RSpec.describe HintHelper, type: :helper do
  describe "#hint_data" do
    it "returns a hash with the label" do
      expect(helper.hint_data("Reply")).to eq({ hint: "Reply" })
    end

    it "includes hint_key when key is given" do
      expect(helper.hint_data("Reply", key: "r")).to eq({ hint: "Reply", hint_key: "r" })
    end

    it "includes hint_placement when placement is given" do
      expect(helper.hint_data("People", placement: :right)).to eq({ hint: "People", hint_placement: :right })
    end

    it "includes all three when all are given" do
      expect(helper.hint_data("People", key: "g p", placement: :right)).to eq({
        hint: "People", hint_key: "g p", hint_placement: :right
      })
    end

    it "compacts nil values" do
      expect(helper.hint_data("Archive", key: nil, placement: nil)).to eq({ hint: "Archive" })
    end
  end

  describe "#hint_aria" do
    it "returns keyshortcuts hash when key is given" do
      expect(helper.hint_aria("r")).to eq({ keyshortcuts: "r" })
    end

    it "returns empty hash when key is nil" do
      expect(helper.hint_aria(nil)).to eq({})
    end

    it "returns empty hash when key is blank" do
      expect(helper.hint_aria("")).to eq({})
    end

    it "maps modifier symbols" do
      expect(helper.hint_aria("⌘K")).to eq({ keyshortcuts: "Meta+K" })
    end
  end

  describe "#hint_html_attrs" do
    it "returns a flat string-keyed hash" do
      result = helper.hint_html_attrs("Archive", key: "e")
      expect(result["data-hint"]).to eq("Archive")
      expect(result["data-hint-key"]).to eq("e")
      expect(result["aria-keyshortcuts"]).to eq("e")
    end

    it "omits nil keys" do
      result = helper.hint_html_attrs("Star")
      expect(result).not_to have_key("data-hint-key")
      expect(result).not_to have_key("aria-keyshortcuts")
    end

    it "includes placement when given" do
      result = helper.hint_html_attrs("People", key: "g p", placement: :right)
      expect(result["data-hint-placement"]).to eq("right")
    end
  end

  describe "#aria_keyshortcuts_for" do
    it "maps ⌘K to Meta+K" do
      expect(helper.aria_keyshortcuts_for("⌘K")).to eq("Meta+K")
    end

    it "maps ⇧I to Shift+I" do
      expect(helper.aria_keyshortcuts_for("⇧I")).to eq("Shift+I")
    end

    it "maps ⌥X to Alt+X" do
      expect(helper.aria_keyshortcuts_for("⌥X")).to eq("Alt+X")
    end

    it "maps ⌃X to Control+X" do
      expect(helper.aria_keyshortcuts_for("⌃X")).to eq("Control+X")
    end

    it "maps ⏎ to Enter" do
      expect(helper.aria_keyshortcuts_for("⏎")).to eq("Enter")
    end

    it "maps Esc to Escape" do
      expect(helper.aria_keyshortcuts_for("Esc")).to eq("Escape")
    end

    it "maps → to ArrowRight" do
      expect(helper.aria_keyshortcuts_for("→")).to eq("ArrowRight")
    end

    it "maps ← to ArrowLeft" do
      expect(helper.aria_keyshortcuts_for("←")).to eq("ArrowLeft")
    end

    it "returns sequences verbatim" do
      expect(helper.aria_keyshortcuts_for("g p")).to eq("g p")
    end

    it "returns plain letters as-is" do
      expect(helper.aria_keyshortcuts_for("e")).to eq("e")
    end

    it "returns nil for nil input" do
      expect(helper.aria_keyshortcuts_for(nil)).to be_nil
    end

    it "returns nil for blank input" do
      expect(helper.aria_keyshortcuts_for("")).to be_nil
    end
  end
end
