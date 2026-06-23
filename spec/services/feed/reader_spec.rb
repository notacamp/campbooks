require "rails_helper"

RSpec.describe Feed::Reader do
  describe ".group_timeline" do
    def pair(kind) = { item: FeedItem.new(kind: kind), subject: Object.new }

    it "wraps each non-tag pair as its own card entry, in order" do
      pairs = [ pair("email_action"), pair("calendar_event") ]

      groups = described_class.group_timeline(pairs)

      expect(groups.map { |g| g[:type] }).to eq(%i[card card])
    end

    it "collapses a consecutive run of tag_suggestion pairs into one tag_queue group" do
      pairs = [ pair("email_action"), pair("tag_suggestion"), pair("tag_suggestion"), pair("email_action") ]

      groups = described_class.group_timeline(pairs)

      expect(groups.map { |g| g[:type] }).to eq(%i[card tag_queue card])
      expect(groups[1][:items].size).to eq(2)
    end

    it "keeps non-adjacent tag_suggestion runs as separate tag_queue groups" do
      pairs = [ pair("tag_suggestion"), pair("email_action"), pair("tag_suggestion") ]

      groups = described_class.group_timeline(pairs)

      expect(groups.map { |g| g[:type] }).to eq(%i[tag_queue card tag_queue])
    end
  end
end
