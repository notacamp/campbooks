# frozen_string_literal: true

require "spec_helper"
require_relative "../../../app/services/emails/embedding_classifier"

RSpec.describe Emails::EmbeddingClassifier do
  # A stand-in for a `neighbor`-gem query result: responds to #tag and
  # #neighbor_distance (cosine distance).
  def neighbor(distance, group_name)
    tag = Struct.new(:group_name).new(group_name)
    Struct.new(:neighbor_distance, :tag).new(distance, tag)
  end

  describe ".verdicts_from" do
    it "ranks candidates by similarity, nearest first" do
      list = described_class.verdicts_from([ neighbor(0.40, "Promos"), neighbor(0.10, "Finance"), neighbor(0.25, "Notifications") ])
      expect(list.map(&:group_name)).to eq([ "Finance", "Notifications", "Promos" ])
      expect(list.map { |v| v.similarity.round(2) }).to eq([ 0.90, 0.75, 0.60 ])
    end
  end

  describe ".best_verdict" do
    it "returns nil when there are no neighbors" do
      expect(described_class.best_verdict([])).to be_nil
    end

    it "picks the nearest tag and converts cosine distance to similarity" do
      verdict = described_class.best_verdict([ neighbor(0.30, "Promos"), neighbor(0.10, "Notifications") ])
      expect(verdict.group_name).to eq("Notifications")
      expect(verdict.similarity).to be_within(0.0001).of(0.90)
    end

    it "is confident about a close match and unsure about a far one" do
      close = described_class.best_verdict([ neighbor(0.10, "Finance") ])
      far   = described_class.best_verdict([ neighbor(0.40, "Finance") ])
      expect(close.confident?).to be(true)   # similarity 0.90 >= 0.78
      expect(far.confident?).to be(false)    # similarity 0.60 <  0.78
    end
  end
end
