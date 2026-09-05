# frozen_string_literal: true

require "rails_helper"

RSpec.describe People::Priority do
  let(:now) { Time.zone.local(2026, 9, 3, 12, 0, 0) }

  def standing(needs_you: false, kind: :none)
    People::Standing::Result.new(needs_you: needs_you, kind: kind)
  end

  def needing = standing(needs_you: true, kind: :attention)
  def recent  = standing

  # Facts with sensible zeros; override what a case is about.
  def facts(**overrides)
    described_class::Facts.new(
      **{
        standing: recent, two_way_threads: 0, outbound_threads: 0, email_count: 1,
        starred: false, allowed: false, classified: false, relationship_type: nil,
        item_score: 0.0, last_activity: now - 1.day
      }.merge(overrides)
    )
  end

  def score(f) = described_class.score(f, now: now).value
  def strength(f) = described_class.new(f, now: now).strength

  # A regular correspondent: you've replied in three threads, ~20 mails, classified.
  def regular(**overrides)
    facts(**{ two_way_threads: 3, outbound_threads: 3, email_count: 20, classified: true }.merge(overrides))
  end

  # Someone who wrote once and you never answered.
  def one_off(**overrides) = facts(**overrides)

  describe "strength" do
    it "grows with real exchanges and saturates so no signal runs away" do
      expect(strength(one_off)).to be < strength(facts(two_way_threads: 1, outbound_threads: 1))
      expect(strength(facts(two_way_threads: 1, outbound_threads: 1))).to be < strength(regular)
      expect(strength(regular(two_way_threads: 300, outbound_threads: 300, email_count: 10_000,
                              starred: true, allowed: true, relationship_type: "client"))).to be < 9.5
    end

    it "a star outweighs volume alone" do
      expect(strength(one_off(starred: true))).to be > strength(one_off(email_count: 50))
    end

    it "ranks the relationship label: VIP > known > unknown / self" do
      expect(strength(one_off(relationship_type: "client"))).to be > strength(one_off(relationship_type: "vendor"))
      expect(strength(one_off(relationship_type: "vendor"))).to be > strength(one_off(relationship_type: "unknown"))
      expect(strength(one_off(relationship_type: "unknown"))).to eq(strength(one_off(relationship_type: "self")))
      expect(strength(one_off(relationship_type: "self"))).to eq(strength(one_off))
    end
  end

  describe "attention weight (the learned strength)" do
    it "sets strength to the weight scaled onto the 0..9 band" do
      expect(strength(facts(attention_weight: 0.8))).to eq((0.8 * described_class::ATTENTION_STRENGTH_SCALE).round(4))
      expect(strength(facts(attention_weight: 0.8))).to eq(7.2)
    end

    it "falls back to the legacy strength when there is no weight (nil)" do
      expect(strength(regular(attention_weight: nil))).to eq(strength(regular))
    end

    it "uses a zero weight as a real zero strength (not a fallback)" do
      expect(strength(regular(attention_weight: 0.0))).to eq(0.0)
    end

    it "orders two equal-item_score Need-you rows by their weight" do
      high = regular(standing: needing, item_score: 50.0, attention_weight: 0.9)
      low  = regular(standing: needing, item_score: 50.0, attention_weight: 0.2)
      expect(score(high)).to be > score(low)
    end

    it "orders two equally-recent Recent rows by their weight" do
      high = facts(last_activity: now - 2.days, attention_weight: 0.9)
      low  = facts(last_activity: now - 2.days, attention_weight: 0.2)
      expect(score(high)).to be > score(low)
    end
  end

  describe "Need you (obligation = item_score + strength)" do
    it "a genuine ask from a regular outranks a stranger's with a lower-scored item" do
      # regular has much more strength, so even with a somewhat higher item_score on the one_off,
      # at the same item_score the regular wins.
      expect(score(regular(standing: needing, item_score: 50.0))).to \
        be > score(one_off(standing: needing, item_score: 50.0))
    end

    it "between equals a higher item_score wins" do
      expect(score(regular(standing: needing, item_score: 70.0))).to \
        be > score(regular(standing: needing, item_score: 50.0))
    end

    it "a higher item_score can beat a stranger with a lower one even at lower strength" do
      # one_off with item_score 100 beats regular with item_score 20
      # (100 + low_strength) > (20 + high_strength ~5)
      expect(score(one_off(standing: needing, item_score: 100.0))).to \
        be > score(regular(standing: needing, item_score: 20.0))
    end

    it "flags needs_you on the score" do
      expect(described_class.score(regular(standing: needing, item_score: 50.0), now: now).needs_you).to be true
      expect(described_class.score(regular, now: now).needs_you).to be false
    end
  end

  describe "Recent" do
    it "halves recency every 14 days and reads 0 with no mail at all" do
      r = ->(f) { described_class.new(f, now: now).recency }
      expect(r.(facts(last_activity: now))).to be_within(0.001).of(1.0)
      expect(r.(facts(last_activity: now - 14.days))).to be_within(0.001).of(0.5)
      expect(r.(facts(last_activity: now - 28.days))).to be_within(0.001).of(0.25)
      expect(r.(facts(last_activity: nil))).to eq(0.0)
    end

    it "a week-old regular beats yesterday's one-off; a month-old regular does not" do
      expect(score(regular(last_activity: now - 7.days))).to be > score(one_off(last_activity: now - 1.day))
      expect(score(regular(last_activity: now - 30.days))).to be < score(one_off(last_activity: now - 1.day))
    end

    it "is recency-led among equals" do
      expect(score(regular(last_activity: now - 1.day))).to be > score(regular(last_activity: now - 5.days))
    end
  end

  describe ".facts_for" do
    it "derives the relationship facts from loaded threads and contacts" do
      threads = [
        build_stubbed(:email_thread, last_inbound_at: now - 3.days, last_outbound_at: now - 2.days),
        build_stubbed(:email_thread, last_inbound_at: now - 9.days, last_outbound_at: nil),
        build_stubbed(:email_thread, last_inbound_at: nil, last_outbound_at: now - 20.days),
        build_stubbed(:email_thread, last_inbound_at: now - 9.days, last_outbound_at: now - 6.days)
      ]
      contacts = [
        build_stubbed(:contact, email_count: 12, starred_at: now, list_status: :allowed, sender_kind_source: "heuristic"),
        build_stubbed(:contact, email_count: 3, sender_kind_source: nil)
      ]

      f = described_class.facts_for(standing: standing, threads: threads, contacts: contacts,
                                    relationship_type: "client", last_activity: now - 2.days,
                                    item_score: 55.0, now: now)

      expect(f.two_way_threads).to eq(2)
      expect(f.outbound_threads).to eq(3)
      expect(f.email_count).to eq(15)
      expect(f.item_score).to eq(55.0)
      expect(f).to have_attributes(starred: true, allowed: true, classified: true, relationship_type: "client")
    end

    it "returns zero/false for an empty one-off with no item" do
      bare = described_class.facts_for(standing: standing, threads: [], contacts: [],
                                       relationship_type: nil, last_activity: nil, now: now)
      expect(bare).to have_attributes(email_count: 0, item_score: 0.0, starred: false,
                                      classified: false, relationship_type: nil, attention_weight: nil)
    end

    it "carries the attention weight through when given" do
      f = described_class.facts_for(standing: standing, threads: [], contacts: [],
                                    relationship_type: nil, last_activity: nil, attention_weight: 0.7)
      expect(f.attention_weight).to eq(0.7)
    end
  end
end
