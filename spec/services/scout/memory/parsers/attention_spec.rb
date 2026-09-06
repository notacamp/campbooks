# frozen_string_literal: true

require "rails_helper"

RSpec.describe Scout::Memory::Parsers::Attention do
  def parse(text) = described_class.call(text)

  describe "important matchers" do
    it "matches '<name> is important'" do
      result = parse("Sofia is important")
      expect(result).to eq({ kind: :attention, contact: "Sofia", label: "important" })
    end

    it "matches 'mail from <email> is important'" do
      result = parse("mail from sofia@x.com is important")
      expect(result).to eq({ kind: :attention, contact: "sofia@x.com", label: "important" })
    end

    it "matches '<name> is very important'" do
      result = parse("Ana is very important")
      expect(result).to eq({ kind: :attention, contact: "Ana", label: "important" })
    end

    it "matches '<name> are important'" do
      result = parse("the partners are important")
      expect(result).to eq({ kind: :attention, contact: "the partners", label: "important" })
    end

    it "matches '<name> matters'" do
      result = parse("Sofia matters")
      expect(result).to eq({ kind: :attention, contact: "Sofia", label: "important" })
    end

    it "matches '<name> matter to me'" do
      result = parse("Ana and Luis matter to me")
      expect(result).to eq({ kind: :attention, contact: "Ana and Luis", label: "important" })
    end

    it "matches 'prioritize <name>'" do
      result = parse("prioritize Sofia")
      expect(result).to eq({ kind: :attention, contact: "Sofia", label: "important" })
    end

    it "matches 'pay attention to <name>'" do
      result = parse("pay attention to Ana Reis")
      expect(result).to eq({ kind: :attention, contact: "Ana Reis", label: "important" })
    end

    it "matches 'always prioritise <name>'" do
      result = parse("always prioritise the board")
      expect(result).to eq({ kind: :attention, contact: "the board", label: "important" })
    end

    it "does not match a question 'is Sofia important?'" do
      expect(parse("is Sofia important?")).to be_nil
    end
  end

  describe "unimportant matchers" do
    it "matches '<name> is not important'" do
      result = parse("newsletters are not important")
      expect(result).to eq({ kind: :attention, contact: "newsletters", label: "unimportant" })
    end

    it "matches 'mail from <name> is not important'" do
      result = parse("mail from spam@x.com is not important")
      expect(result).to eq({ kind: :attention, contact: "spam@x.com", label: "unimportant" })
    end

    it "matches '<name> doesn't matter'" do
      result = parse("X doesn't matter")
      expect(result).to eq({ kind: :attention, contact: "X", label: "unimportant" })
    end

    it "matches '<name> does not matter'" do
      result = parse("that newsletter does not matter")
      expect(result).to eq({ kind: :attention, contact: "that newsletter", label: "unimportant" })
    end

    it "matches 'ignore <name>'" do
      result = parse("ignore newsletters")
      expect(result).to eq({ kind: :attention, contact: "newsletters", label: "unimportant" })
    end

    it "matches 'deprioritize <name>'" do
      result = parse("deprioritize promotions")
      expect(result).to eq({ kind: :attention, contact: "promotions", label: "unimportant" })
    end

    it "matches 'keep <name> out of the way'" do
      result = parse("keep newsletters out of the way")
      expect(result).to eq({ kind: :attention, contact: "newsletters", label: "unimportant" })
    end
  end

  describe "non-matching sentences" do
    it "returns nil for an unrelated sentence" do
      expect(parse("make me a sandwich")).to be_nil
    end

    it "returns nil for blank input" do
      expect(parse("")).to be_nil
    end

    it "returns nil for 'is priority' sentences (handled by Priority parser)" do
      expect(parse("sofia@x.com is priority")).to be_nil
    end
  end
end
