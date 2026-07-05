require "rails_helper"

RSpec.describe Feed::Reader do
  def pair(kind) = { item: FeedItem.new(kind: kind), subject: Object.new }

  describe ".group_timeline" do
    it "wraps each non-tag pair as its own card entry, in order" do
      pairs = [ pair("email_action"), pair("calendar_event") ]

      groups = described_class.group_timeline(pairs)

      expect(groups.map { |g| g[:type] }).to eq(%i[card card])
    end

    it "collapses the page's tag_suggestion pairs into one tag_queue at the tail" do
      pairs = [ pair("email_action"), pair("tag_suggestion"), pair("tag_suggestion"), pair("email_action") ]

      groups = described_class.group_timeline(pairs)

      expect(groups.map { |g| g[:type] }).to eq(%i[card card tag_queue])
      expect(groups.last[:items].size).to eq(2)
    end

    it "sweeps stray tag_suggestions into that same tail queue" do
      pairs = [ pair("tag_suggestion"), pair("email_action"), pair("tag_suggestion") ]

      groups = described_class.group_timeline(pairs)

      expect(groups.map { |g| g[:type] }).to eq(%i[card tag_queue])
      expect(groups.last[:items].size).to eq(2)
    end
  end

  describe ".interleave" do
    def kinds(pairs) = described_class.interleave(pairs).map { |p| p[:item].kind }

    it "caps a same-kind run by pulling the next different kind forward" do
      pairs = [ pair("email_action"), pair("email_action"), pair("email_action"), pair("follow_up") ]

      expect(kinds(pairs)).to eq(%w[email_action email_action follow_up email_action])
    end

    it "passes an all-one-kind page through unchanged" do
      pairs = Array.new(4) { pair("email_action") }

      expect(described_class.interleave(pairs)).to eq(pairs)
    end

    it "never emits a run longer than the cap while a break exists" do
      pairs = %w[a a a b b b a].map { |k| pair(k) }

      runs = kinds(pairs).chunk_while { |x, y| x == y }.map(&:size)

      expect(runs.max).to be <= Feed::Reader::MAX_KIND_RUN
    end

    it "keeps rank order except at run breaks" do
      pairs = [ pair("a"), pair("b"), pair("a"), pair("b") ]

      expect(described_class.interleave(pairs)).to eq(pairs)
    end
  end
end
