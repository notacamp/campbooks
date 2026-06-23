# frozen_string_literal: true

require "spec_helper"
require_relative "../../../app/services/emails/llm_tag_picker"

RSpec.describe Emails::LlmTagPicker do
  def tag(name) = Struct.new(:name).new(name)

  # Inject a canned model reply so the selection logic is tested without an LLM.
  def pick(candidates, reply)
    described_class.new(:email, candidates, completion: ->(_email, _tags) { reply }).call
  end

  describe "#call" do
    it "returns the tag the model chose (1-based)" do
      a, b, c = tag("Promos"), tag("Finance"), tag("Personal")
      expect(pick([ a, b, c ], "2").tag).to eq(b)
    end

    it "abstains when the model replies 0" do
      expect(pick([ tag("A"), tag("B") ], "0")).to be_nil
    end

    it "abstains on a non-numeric reply" do
      expect(pick([ tag("A") ], "none of these")).to be_nil
    end

    it "abstains when the choice is out of range" do
      expect(pick([ tag("A"), tag("B") ], "5")).to be_nil
    end

    it "returns nil with no candidates" do
      expect(pick([], "1")).to be_nil
    end

    it "accepts verdict-like candidates that respond to #tag" do
      a = tag("Promos")
      verdict = Struct.new(:tag).new(a)
      expect(pick([ verdict ], "1").tag).to eq(a)
    end
  end

  describe ".parse_choice" do
    it "extracts a valid 1-based index" do
      expect(described_class.parse_choice("2", 3)).to eq(2)
      expect(described_class.parse_choice("Tag 3 fits best", 3)).to eq(3)
    end

    it "returns nil for 0 / out-of-range / no digits" do
      expect(described_class.parse_choice("0", 3)).to be_nil
      expect(described_class.parse_choice("9", 3)).to be_nil
      expect(described_class.parse_choice("none", 3)).to be_nil
    end
  end
end
