require "rails_helper"

RSpec.describe FeedItem, type: :model do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }
  let(:account) { create(:email_account, workspace: workspace) }
  let(:message) { create(:email_message, email_account: account) }

  def build_item(**overrides)
    FeedItem.create!({
      user: user, workspace: workspace, kind: "email_action", subject: message,
      dedupe_key: "email_action:#{message.id}", sort_at: Time.current, generated_at: Time.current
    }.merge(overrides))
  end

  describe "scopes" do
    it "active excludes dismissed and acted items" do
      live = build_item
      build_item(dedupe_key: "x:1", dismissed_at: Time.current)
      build_item(dedupe_key: "x:2", acted_at: Time.current)

      expect(FeedItem.active).to contain_exactly(live)
    end

    it "splits attention from timeline" do
      pinned = build_item(dedupe_key: "a", attention: true)
      streamed = build_item(dedupe_key: "b", attention: false)

      expect(FeedItem.attention).to contain_exactly(pinned)
      expect(FeedItem.timeline).to contain_exactly(streamed)
    end

    it "orders timeline newest-first and attention by score" do
      older = build_item(dedupe_key: "old", sort_at: 2.days.ago)
      newer = build_item(dedupe_key: "new", sort_at: 1.hour.ago)
      expect(FeedItem.chronological.to_a).to eq([ newer, older ])

      low = build_item(dedupe_key: "low", attention: true, score: 10)
      high = build_item(dedupe_key: "high", attention: true, score: 90)
      expect(FeedItem.attention.ranked.to_a.first(2)).to eq([ high, low ])
    end
  end

  describe "state transitions" do
    it "marks seen / dismissed / acted and reports active?" do
      item = build_item
      expect(item).to be_active

      item.mark_seen!
      expect(item.reload.seen_at).to be_present
      expect(item).to be_active # seen is not a resolution

      item.dismiss!
      expect(item.reload).not_to be_active
    end
  end
end
