# frozen_string_literal: true

require "rails_helper"

RSpec.describe People::Priority do
  let(:now) { Time.zone.local(2026, 9, 3, 12, 0, 0) }

  def standing(needs_you: false, kind: :none, overdue_days: 0, thread_id: nil)
    People::Standing::Result.new(text: "x", needs_you: needs_you, thread_id: thread_id, overdue_days: overdue_days, kind: kind)
  end

  def owe(days) = standing(needs_you: true, kind: :you_owe, overdue_days: days)
  def nudge(days) = standing(needs_you: true, kind: :nudge, overdue_days: days)

  # Facts with sensible zeros; override what a case is about.
  def facts(**overrides)
    described_class::Facts.new(
      **{
        standing: standing, two_way_threads: 0, outbound_threads: 0, email_count: 1,
        starred: false, allowed: false, classified: false, relationship_type: nil,
        action_prompt: false, important: false, follow_up_due: false, follow_up_vetted: false,
        last_activity: now - 1.day
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

  describe "Need you" do
    it "a genuine ask from a regular outranks a stranger's older one" do
      expect(score(regular(standing: owe(2)))).to be > score(one_off(standing: owe(14)))
    end

    it "between equals, the one waiting longer comes first" do
      expect(score(regular(standing: owe(9)))).to be > score(regular(standing: owe(2)))
    end

    it "a reply you owe — even a fresh one — outranks a nudge you could send, even a long-overdue one" do
      expect(score(regular(standing: owe(1)))).to be > score(regular(standing: nudge(19), follow_up_vetted: true))
    end

    it "a nudge the AI vetted outranks one it hasn't" do
      expect(score(regular(standing: nudge(6), follow_up_vetted: true))).to be > score(regular(standing: nudge(6)))
    end

    it "an unvetted nudge to a VIP still sits below a stranger's genuine ask" do
      vip_nudge = regular(standing: nudge(6), relationship_type: "colleague")
      expect(score(one_off(standing: owe(30), classified: true))).to be > score(vip_nudge)
    end

    it "a Scout prompt, an important read and a due follow-up each add a bonus" do
      base = regular(standing: owe(3))
      expect(score(base.with(action_prompt: true))).to be > score(base)
      expect(score(base.with(important: true))).to be > score(base)
      expect(score(base.with(follow_up_due: true))).to be > score(base)
    end

    it "flags needs_you on the score" do
      expect(described_class.score(regular(standing: owe(1)), now: now).needs_you).to be true
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
    it "derives the relationship facts from loaded threads, contacts and the latest inbound" do
      nudged = build_stubbed(:email_thread, last_inbound_at: now - 9.days, last_outbound_at: now - 6.days,
                                            follow_up_last_analyzed_at: now - 5.days)
      threads = [
        build_stubbed(:email_thread, last_inbound_at: now - 3.days, last_outbound_at: now - 2.days),
        build_stubbed(:email_thread, last_inbound_at: now - 9.days, last_outbound_at: nil),
        build_stubbed(:email_thread, last_inbound_at: nil, last_outbound_at: now - 20.days,
                                     follow_up_expected: true, follow_up_at: now - 1.day),
        nudged
      ]
      contacts = [
        build_stubbed(:contact, email_count: 12, starred_at: now, list_status: :allowed, sender_kind_source: "heuristic"),
        build_stubbed(:contact, email_count: 3, sender_kind_source: nil)
      ]
      latest = build_stubbed(:email_message, ai_action_prompt: "Reply by Friday.", ai_priority: :high)

      f = described_class.facts_for(standing: standing(needs_you: true, kind: :nudge, overdue_days: 6, thread_id: nudged.id),
                                    threads: threads, contacts: contacts, latest_inbound: latest,
                                    relationship_type: "client", last_activity: now - 2.days, now: now)

      expect(f.two_way_threads).to eq(2)
      expect(f.outbound_threads).to eq(3)
      expect(f.email_count).to eq(15)
      expect(f).to have_attributes(starred: true, allowed: true, classified: true, relationship_type: "client",
                                   action_prompt: true, important: true, follow_up_due: true, follow_up_vetted: true)
    end

    it "reads an important category as important, an unvetted nudge as unvetted, and blank facts for a bare one-off" do
      latest = build_stubbed(:email_message, category: "important", ai_priority: :medium)
      unvetted = build_stubbed(:email_thread, last_inbound_at: now - 9.days, last_outbound_at: now - 6.days)
      f = described_class.facts_for(standing: standing(needs_you: true, kind: :nudge, overdue_days: 6, thread_id: unvetted.id),
                                    threads: [ unvetted ], contacts: [ build_stubbed(:contact, email_count: 1) ],
                                    latest_inbound: latest, relationship_type: "", last_activity: nil, now: now)
      expect(f).to have_attributes(two_way_threads: 1, outbound_threads: 1, email_count: 1, starred: false,
                                   allowed: false, classified: false, relationship_type: nil, action_prompt: false,
                                   important: true, follow_up_due: false, follow_up_vetted: false, last_activity: nil)

      bare = described_class.facts_for(standing: standing, threads: [], contacts: [], latest_inbound: nil,
                                       relationship_type: nil, last_activity: nil, now: now)
      expect(bare).to have_attributes(email_count: 0, important: false, follow_up_vetted: false)
    end
  end
end
