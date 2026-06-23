# frozen_string_literal: true

require "spec_helper"
require_relative "../../../app/services/emails/clusterer"

RSpec.describe Emails::Clusterer do
  # Three obvious "topics" in 3-D space — think CI builds, PR threads, a promo.
  let(:items) do
    [
      [ :ci1,   [ 1.0, 0.0, 0.0 ] ],
      [ :ci2,   [ 0.98, 0.02, 0.0 ] ],
      [ :ci3,   [ 0.95, 0.0, 0.05 ] ],
      [ :pr1,   [ 0.0, 1.0, 0.0 ] ],
      [ :pr2,   [ 0.01, 0.99, 0.0 ] ],
      [ :promo, [ 0.0, 0.0, 1.0 ] ]
    ]
  end

  it "collapses near-identical vectors into one stack each, biggest first" do
    clusters = described_class.cluster(items, min_similarity: 0.9)
    expect(clusters.size).to eq(3)
    expect(clusters.map(&:size)).to eq([ 3, 2, 1 ])
  end

  it "keeps members of the same topic together" do
    clusters = described_class.cluster(items, min_similarity: 0.9)
    ci = clusters.find { |c| c.member_ids.include?(:ci1) }
    expect(ci.member_ids).to contain_exactly(:ci1, :ci2, :ci3)
  end

  it "splits more aggressively as the threshold rises" do
    loose = described_class.cluster(items, min_similarity: 0.5).size
    tight = described_class.cluster(items, min_similarity: 0.999).size
    expect(tight).to be > loose
  end

  it "skips empty / nil vectors" do
    clusters = described_class.cluster([ [ :a, [] ], [ :b, nil ], [ :c, [ 1.0, 0.0 ] ] ])
    expect(clusters.flat_map(&:member_ids)).to eq([ :c ])
  end
end
