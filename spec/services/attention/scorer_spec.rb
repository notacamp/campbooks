# frozen_string_literal: true

require "rails_helper"

RSpec.describe Attention::Scorer do
  def facts(**overrides)
    Attention::Facts.blank(**overrides)
  end

  def score(f) = described_class.score(f)
  def terms(f) = described_class.new(f).terms

  describe "Facts.blank default" do
    it "produces weight 0.3, confidence 0.0, and reason [new]" do
      result = score(facts)
      expect(result.weight).to be_within(0.001).of(0.3)
      expect(result.confidence).to eq(0.0)
      expect(result.reasons.map(&:key)).to eq(%w[new])
    end
  end

  describe "regular correspondent" do
    let(:regular_facts) do
      facts(
        replied_count: 4, median_reply_hours: 6, two_way_threads: 3,
        outbound_threads: 3, addressed_count: 20, inbound_count: 24,
        opened_count: 20, meetings_count: 1
      )
    end

    it "scores weight >= 0.85 and confidence >= 0.9" do
      result = score(regular_facts)
      expect(result.weight).to be >= 0.85
      expect(result.confidence).to be >= 0.9
    end

    it "names replies_fast as the first reason" do
      result = score(regular_facts)
      expect(result.reasons.first.key).to eq("replies_fast")
    end
  end

  describe "newsletter / service" do
    let(:newsletter_facts) do
      facts(inbound_count: 40, opened_count: 5, archived_unread_count: 20, sender_kind: "service")
    end

    it "scores weight <= 0.08" do
      result = score(newsletter_facts)
      expect(result.weight).to be <= 0.08
    end

    it "includes ignored and service in reasons" do
      result = score(newsletter_facts)
      reason_keys = result.reasons.map(&:key)
      expect(reason_keys).to include("ignored")
      expect(reason_keys).to include("service")
    end
  end

  describe "vendor you pay" do
    let(:vendor_facts) do
      facts(
        invoices_count: 6, settled_count: 6, median_settle_delay_days: -1,
        inbound_count: 8, opened_count: 6, addressed_count: 6, relationship_type: "vendor"
      )
    end

    it "scores strictly between newsletter and regular" do
      newsletter_w = score(facts(inbound_count: 40, opened_count: 5, archived_unread_count: 20, sender_kind: "service")).weight
      regular_w    = score(facts(replied_count: 4, median_reply_hours: 6, two_way_threads: 3, outbound_threads: 3, addressed_count: 20, inbound_count: 24, opened_count: 20, meetings_count: 1)).weight
      vendor_w     = score(vendor_facts).weight
      expect(vendor_w).to be > newsletter_w
      expect(vendor_w).to be < regular_w
    end

    it "includes invoices and pays_promptly in reasons" do
      result = score(vendor_facts)
      reason_keys = result.reasons.map(&:key)
      expect(reason_keys).to include("invoices")
      expect(reason_keys).to include("pays_promptly")
    end
  end

  describe "stranger with one message" do
    it "scores within 0.02 of 0.3" do
      result = score(facts(inbound_count: 1))
      expect(result.weight).to be_within(0.02).of(0.3)
    end
  end

  describe "starred stranger" do
    it "scores weight >= 0.8, confidence >= 0.6, first reason starred" do
      result = score(facts(starred: true))
      expect(result.weight).to be >= 0.8
      expect(result.confidence).to be >= 0.6
      expect(result.reasons.first.key).to eq("starred")
    end
  end

  describe "taught unimportant on a regular correspondent" do
    it "clamps weight <= 0.1 and includes taught_unimportant" do
      f = facts(
        replied_count: 4, median_reply_hours: 6, two_way_threads: 3,
        outbound_threads: 3, addressed_count: 20, inbound_count: 24,
        opened_count: 20, taught: "unimportant"
      )
      result = score(f)
      expect(result.weight).to be <= 0.1
      expect(result.reasons.map(&:key)).to include("taught_unimportant")
    end

    it "taught_unimportant wins over starred (ceiling beats floor)" do
      f = facts(starred: true, taught: "unimportant")
      result = score(f)
      expect(result.weight).to be <= 0.1
      expect(result.reasons.map(&:key)).to include("taught_unimportant")
    end
  end

  describe "blocked" do
    it "returns weight 0.0, confidence 1.0, reasons [blocked] regardless of other facts" do
      f = facts(
        blocked: true, replied_count: 10, starred: true, taught: "important"
      )
      result = score(f)
      expect(result.weight).to eq(0.0)
      expect(result.confidence).to eq(1.0)
      expect(result.reasons.map(&:key)).to eq(%w[blocked])
    end
  end

  describe "reply speed ordering" do
    it "faster replies produce a higher weight than slower" do
      fast = score(facts(replied_count: 3, median_reply_hours: 2, inbound_count: 5, addressed_count: 5))
      slow = score(facts(replied_count: 3, median_reply_hours: 40, inbound_count: 5, addressed_count: 5))
      expect(fast.weight).to be > slow.weight
    end
  end

  describe "dismissals" do
    it "more dismissals produce a lower weight" do
      base  = facts(feed_acted_count: 1, inbound_count: 5)
      fewer = base
      more  = facts(feed_dismissed_count: 4, feed_acted_count: 1, inbound_count: 5)
      expect(score(more).weight).to be < score(fewer).weight
    end
  end

  describe "absurd counts" do
    it "never overflows weight or confidence beyond 1.0" do
      f = facts(
        inbound_count: 10_000, addressed_count: 10_000, replied_count: 10_000,
        median_reply_hours: 0.1, two_way_threads: 10_000, outbound_threads: 10_000,
        opened_count: 10_000, meetings_count: 10_000, invoices_count: 10_000,
        settled_count: 10_000, snoozed_count: 10_000, forwarded_count: 10_000,
        tagged_count: 10_000, feed_acted_count: 10_000
      )
      result = score(f)
      expect(result.weight).to be <= 1.0
      expect(result.confidence).to be <= 1.0
    end
  end

  describe "#terms" do
    it "returns a Hash of Float contributions" do
      f = facts(replied_count: 2, median_reply_hours: 6, inbound_count: 5, addressed_count: 5)
      t = described_class.new(f).terms
      expect(t).to be_a(Hash)
      expect(t.values).to all(be_a(Float))
      expect(t[:replies]).to be > 0
    end
  end
end
