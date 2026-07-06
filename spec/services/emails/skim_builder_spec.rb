# frozen_string_literal: true

require "spec_helper"
# This spec runs in isolation (spec_helper, not rails_helper), so it must require
# the ActiveSupport core extensions its subject uses: present?/blank? (blank),
# humanize (inflections). Without these the builder raises NoMethodError here.
require "active_support/core_ext/object/blank"
require "active_support/core_ext/string/inflections"
require_relative "../../../app/services/emails/categorizer"
require_relative "../../../app/services/emails/sender_domain"
require_relative "../../../app/services/emails/skim_builder"

RSpec.describe Emails::SkimBuilder do
  # Fixed "now" so day-bucketing is deterministic.
  let(:now) { Time.new(2026, 6, 20, 12, 0, 0) }

  ContactStub = Struct.new(:relationship_type)
  EmailStub = Struct.new(
    :id, :from_address, :subject, :summary, :ai_summary, :received_at,
    :read, :pinned_at, :email_thread_id, :contact,
    keyword_init: true
  )

  def email(from, subject, id: nil, summary: nil, ai_summary: nil, at: nil, read: false,
            pinned_at: nil, thread_id: nil, relationship: nil)
    EmailStub.new(
      id: id, from_address: from, subject: subject, summary: summary, ai_summary: ai_summary,
      received_at: at || now, read: read, pinned_at: pinned_at,
      email_thread_id: thread_id,
      contact: (relationship ? ContactStub.new(relationship) : nil)
    )
  end

  def rings_for(emails) = described_class.new(emails, now: now).rings
  def ring(emails, theme) = rings_for(emails).find { |r| r[:theme] == theme }

  describe "theme rings (the menu)" do
    # Bare sender addresses so Emails::Categorizer's domain rules apply cleanly.
    let(:emails) do
      [
        email("jamie@gmail.com", "Lunch tomorrow?"),
        email("news@shop.com", "50% off everything"),
        email("no-reply@github.com", "Build passed for main")
      ]
    end

    it "groups the inbox into theme rings, most-actionable first" do
      expect(rings_for(emails).map { |r| r[:theme] }).to eq(%i[personal notifications promotions])
    end

    it "labels themes for the menu" do
      expect(rings_for(emails).map { |r| r[:label] }).to eq([ "People", "Notifications", "Newsletters & promos" ])
    end
  end

  describe "priority lane" do
    it "puts pinned mail in a leading Priority ring, regardless of theme/age" do
      emails = [
        email("Jamie <jamie@gmail.com>", "Lunch?", at: now),
        email("Shop <news@shop.com>", "Old promo", at: now - (40 * 24 * 3600), pinned_at: now)
      ]
      expect(rings_for(emails).first[:theme]).to eq(:priority)
    end
  end

  describe "time order within a theme (the walk)" do
    it "walks a theme today → yesterday → this week → earlier" do
      emails = [
        email("Deals <news@deals.com>", "Old sale",   at: now - (20 * 24 * 3600)),
        email("Shop <news@shop.com>",   "Fresh sale", at: now),
        email("Mall <news@mall.com>",   "Week sale",  at: now - (4 * 24 * 3600))
      ]
      buckets = ring(emails, :promotions)[:clusters].map { |c| c[:bucket] }
      expect(buckets).to eq(%i[today this_week earlier])
    end
  end

  describe "clustering within a theme" do
    it "collapses same-sender same-topic mail into one stack; the ring badge counts steps, not emails" do
      emails = Array.new(3) { email("CircleCI <no-reply@circleci.com>", "[CircleCI] Workflow failed: x") }
      notif_ring = ring(emails, :notifications)
      cards = notif_ring[:clusters]
      expect(cards.size).to eq(1)
      expect(cards.first[:count]).to eq(3) # the one card holds 3 emails
      # Ring badge = number of skim STEPS (one stack), NOT the 3 emails inside it.
      expect(notif_ring[:count]).to eq(1)
    end

    it "splits one sender into its distinct topics" do
      emails = [
        email("no-reply@github.com", "Build passed for main"),
        email("no-reply@github.com", "Deploy finished for prod")
      ]
      expect(ring(emails, :notifications)[:clusters].size).to eq(2)
    end

    it "groups a conversation thread into one card even across senders" do
      emails = [
        email("ana@gmail.com", "Re: Budget", at: now, thread_id: 99),
        email("bob@gmail.com", "Re: Budget", at: now, thread_id: 99)
      ]
      card = ring(emails, :personal)[:clusters].first
      expect(card[:count]).to eq(2)
    end
  end

  describe "suggested priority (confirmable cue, never asserted)" do
    it "suggests for a security subject in a recent bucket" do
      card = ring([ email("Bank <no-reply@bank.com>", "Your verification code is 1234", at: now) ], :important)[:clusters].first
      expect(card[:priority_suggested]).to be(true)
    end

    it "suggests for a VIP contact" do
      card = ring([ email("Boss <boss@client.com>", "Quick question", at: now, relationship: "client") ], :personal)[:clusters].first
      expect(card[:priority_suggested]).to be(true)
    end

    it "does not suggest ordinary noise" do
      card = ring([ email("Shop <news@shop.com>", "50% off everything", at: now) ], :promotions)[:clusters].first
      expect(card[:priority_suggested]).to be(false)
    end

    it "does not nag on old mail" do
      old = email("Bank <no-reply@bank.com>", "Your verification code is 1234", at: now - (20 * 24 * 3600))
      card = ring([ old ], :important)[:clusters].first
      expect(card[:priority_suggested]).to be(false)
    end
  end

  describe "card payload" do
    it "carries email ids, time bucket label, and per-email details" do
      rows = [ email("Jamie <jamie@gmail.com>", "Re: Lunch?", id: 7, summary: "Are you  free  at 1pm?", at: now) ]
      card = ring(rows, :personal)[:clusters].first
      expect(card[:email_ids]).to eq([ 7 ])
      expect(card[:bucket_label]).to eq("Today")
      expect(card[:emails].first).to include(id: 7, sender: "Jamie", subject: "Lunch?", snippet: "Are you free at 1pm?")
      expect(card[:emails].first[:received_at]).to eq(now)
    end

    it "stamps per-ring story position and total" do
      rows = [
        email("Shop <news@shop.com>", "Sale A", at: now),
        email("Mall <news@mall.com>", "Sale B", at: now)
      ]
      cards = ring(rows, :promotions)[:clusters]
      expect(cards.map { |c| c[:position] }).to eq([ 1, 2 ])
      expect(cards.map { |c| c[:total] }).to all(eq(2))
    end
  end

  describe "#clusters (flat, legacy)" do
    it "returns every cluster with a global position/total" do
      rows = [
        email("Jamie <jamie@gmail.com>", "Hi", at: now),
        email("Shop <news@shop.com>", "Sale", at: now)
      ]
      flat = described_class.new(rows, now: now).clusters
      expect(flat.map { |c| c[:position] }).to eq([ 1, 2 ])
      expect(flat.map { |c| c[:total] }).to all(eq(2))
    end
  end

  describe "sender list state (starred / pending / blocked)" do
    # Richer contact stub exposing the state the builder reads.
    sender = Struct.new(:starred_at, :status, keyword_init: true) do
      def starred? = !starred_at.nil?
      def neutral? = status.nil? || status == :neutral
      def blocked? = status == :blocked
      def pending? = neutral? && !starred?
    end

    def email_c(from, subject, contact:, id: nil, thread_id: nil)
      EmailStub.new(
        id: id, from_address: from, subject: subject, summary: nil,
        received_at: @now_t, read: false, pinned_at: nil,
        email_thread_id: thread_id, contact: contact
      )
    end

    before { @now_t = now }

    it "promotes starred senders into a leading :starred ring; a starred conversation folds to one card, distinct threads stay separate" do
      starred = sender.new(starred_at: now, status: :neutral)
      rows = [
        email_c("vip@studio.com", "Contract A", id: 1, thread_id: 10, contact: starred),
        email_c("vip@studio.com", "Contract B", id: 2, thread_id: 10, contact: starred),
        email_c("vip@studio.com", "Renewal",    id: 4, thread_id: 11, contact: starred),
        email_c("news@shop.com", "Sale", id: 3, contact: nil)
      ]
      rings = described_class.new(rows, now: now).rings

      expect(rings.first[:theme]).to eq(:starred)
      starred_ring = rings.find { |r| r[:theme] == :starred }
      # Same-thread starred mail (thread 10) folds into one card; the distinct
      # thread (11) is its own — two cards, not three. A starred *conversation*
      # collapses; different conversations never merge. (See SkimBuilder#cluster_key.)
      expect(starred_ring[:clusters].size).to eq(2)
    end

    it "routes undecided senders to :pending only in whitelist mode" do
      neutral = sender.new(starred_at: nil, status: :neutral)
      rows = [ email_c("who@new.com", "Hello", id: 1, contact: neutral) ]

      blacklist = described_class.new(rows, now: now, whitelist_mode: false).rings.map { |r| r[:theme] }
      whitelist = described_class.new(rows, now: now, whitelist_mode: true).rings.map { |r| r[:theme] }

      expect(blacklist).not_to include(:pending)
      expect(whitelist).to include(:pending)
    end

    it "treats unknown (contactless) senders as pending in whitelist mode" do
      rows = [ email_c("stranger@new.com", "Hi", id: 1, contact: nil) ]
      themes = described_class.new(rows, now: now, whitelist_mode: true).rings.map { |r| r[:theme] }
      expect(themes).to eq(%i[pending])
    end

    it "drops blocked senders entirely" do
      blocked = sender.new(starred_at: nil, status: :blocked)
      rows = [ email_c("spam@x.com", "Buy now", id: 1, contact: blocked) ]
      expect(described_class.new(rows, now: now).rings).to be_empty
    end
  end

  describe "single-email summary" do
    def card_for(email_obj)
      described_class.new([ email_obj ], now: now).rings.flat_map { |r| r[:clusters] }.first
    end

    it "leads with the email's own ai_summary when Scout has written one" do
      card = card_for(email("ana@acme.com", "Proposal", id: 1, ai_summary: "Ana sent a proposal to review."))
      expect(card[:summary]).to eq("Ana sent a proposal to review.")
    end

    it "falls back to the per-theme line when there's no ai_summary" do
      card = card_for(email("news@shop.com", "50% off everything", id: 2))
      expect(card[:summary]).to eq(described_class::SUMMARIES[:promotions])
    end

    it "does not use ai_summary for multi-email clusters (those summarize async)" do
      rows = [ email("news@shop.com", "Weekly deals", id: 1, ai_summary: "ignored"),
               email("news@shop.com", "Weekly deals", id: 2, ai_summary: "ignored") ]
      card = described_class.new(rows, now: now).rings.flat_map { |r| r[:clusters] }.find { |c| c[:count] == 2 }
      expect(card[:summary]).to eq(described_class::SUMMARIES[:promotions])
    end
  end

  describe "scout suggestions (injected memory)" do
    # A fake memory that suggests archive for github.com senders and nothing else,
    # matching the real SkimActionMemory#suggestion_for keyword interface.
    let(:memory) do
      Class.new do
        def suggestion_for(contact_id: nil, sender_domain: nil, category: nil)
          { action: "archive", count: 9, total: 10 } if sender_domain == "github.com"
        end
      end.new
    end

    def clusters_for(emails)
      described_class.new(emails, now: now, memory: memory).rings.flat_map { |r| r[:clusters] }
    end

    it "attaches the learned suggestion to matching cards" do
      cluster = clusters_for([ email("no-reply@github.com", "Build passed", id: 1) ]).first
      expect(cluster[:scout_suggestion]).to eq(action: "archive", count: 9, total: 10)
    end

    it "leaves non-matching cards without a suggestion" do
      cluster = clusters_for([ email("news@shop.com", "50% off everything", id: 2) ]).first
      expect(cluster[:scout_suggestion]).to be_nil
    end

    it "never suggests on the Priority lane, even when the sender matches" do
      rings = described_class.new([ email("no-reply@github.com", "Build", id: 3, pinned_at: now) ], now: now, memory: memory).rings
      priority = rings.find { |r| r[:theme] == :priority }
      expect(priority[:clusters].first[:scout_suggestion]).to be_nil
    end

    it "is nil when no memory is injected (the pure default)" do
      cluster = described_class.new([ email("no-reply@github.com", "Build", id: 4) ], now: now)
        .rings.flat_map { |r| r[:clusters] }.first
      expect(cluster[:scout_suggestion]).to be_nil
    end
  end

  describe "follow-ups ring (injected thread ids)" do
    it "pulls follow-up threads into a leading :follow_ups ring, carrying the reason" do
      fu    = email("dana@acme.com", "Re: Budget", id: 50, thread_id: 7)
      other = email("news@shop.com", "50% off", id: 51)

      rings = described_class.new(
        [ fu, other ], now: now,
        follow_up_thread_ids: Set[7],
        follow_up_meta: { 7 => { reason: "Confirm the date", at: now - (2 * 24 * 3600) } }
      ).rings

      expect(rings.first[:theme]).to eq(:follow_ups)
      card = rings.first[:clusters].first
      expect(card[:follow_up]).to be(true)
      expect(card[:follow_up_reason]).to eq("Confirm the date")
      # only the follow-up thread's email lands in the ring; the promo stays elsewhere.
      expect(rings.first[:clusters].flat_map { |c| c[:email_ids] }).to eq([ 50 ])
      expect(rings.map { |r| r[:theme] }).to include(:promotions)
    end

    it "does not create a follow-ups ring when no thread ids are injected (pure default)" do
      rings = described_class.new([ email("dana@acme.com", "Re: Budget", id: 52, thread_id: 7) ], now: now).rings
      expect(rings.map { |r| r[:theme] }).not_to include(:follow_ups)
    end
  end
end
