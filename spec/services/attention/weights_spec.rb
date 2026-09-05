# frozen_string_literal: true

require "rails_helper"

RSpec.describe Attention::Weights do
  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace) }
  let(:now)       { Time.zone.local(2026, 9, 5, 12, 0, 0) }

  def weights = described_class.new(user)

  def make_aw(subject, weight: 0.7, confidence: 0.8, computed_at: now)
    t = subject.class.name
    AttentionWeight.create!(
      user: user, workspace: workspace,
      subject_type: t, subject_id: subject.id,
      weight: weight, confidence: confidence, raw_score: 1.0,
      reasons: [], evidence: {}, computed_at: computed_at
    )
  end

  describe "#for" do
    it "returns nil when no row exists" do
      person = create(:person)
      expect(weights.for(person)).to be_nil
    end

    it "returns the AttentionWeight when a row exists" do
      person = create(:person)
      aw = make_aw(person)
      expect(weights.for(person)).to eq(aw)
    end

    it "memoizes the result so subsequent calls make no query" do
      person = create(:person)
      make_aw(person)
      w = weights
      w.for(person) # prime cache

      query_count = 0
      callback = ->(_name, _start, _finish, _id, payload) {
        next if payload[:name].to_s =~ /SCHEMA|TRANSACTION|SAVEPOINT/i
        query_count += 1
      }
      ActiveSupport::Notifications.subscribed(callback, "sql.active_record") do
        w.for(person)
      end
      expect(query_count).to eq(0)
    end
  end

  describe "#weight" do
    it "returns DEFAULT_WEIGHT when no row exists" do
      person = create(:person)
      expect(weights.weight(person)).to eq(described_class::DEFAULT_WEIGHT)
    end

    it "returns the stored weight when a row exists" do
      person = create(:person)
      make_aw(person, weight: 0.9)
      expect(weights.weight(person)).to be_within(0.001).of(0.9)
    end
  end

  describe "#contacts" do
    it "maps contact_id -> AttentionWeight via person_id" do
      person = create(:person)
      contact = create(:contact, person: person, workspace: workspace)
      aw = make_aw(person)

      result = weights.contacts([ contact.id ])
      expect(result[contact.id]).to eq(aw)
    end
  end

  describe "#missing?" do
    it "returns true when no rows exist" do
      expect(weights.missing?).to be true
    end

    it "returns false when rows exist" do
      make_aw(create(:person))
      expect(weights.missing?).to be false
    end
  end

  describe "#stale?" do
    it "returns true when missing" do
      expect(weights.stale?).to be true
    end

    it "returns false when a fresh row exists within threshold" do
      make_aw(create(:person), computed_at: 5.minutes.ago)
      expect(weights.stale?(threshold: 15.minutes)).to be false
    end

    it "returns true when all rows are older than threshold" do
      make_aw(create(:person), computed_at: 20.minutes.ago)
      expect(weights.stale?(threshold: 15.minutes)).to be true
    end
  end

  describe "#preload then no further queries" do
    it "loads records in one query then serves from cache" do
      p1 = create(:person)
      p2 = create(:person)
      make_aw(p1, weight: 0.5)
      make_aw(p2, weight: 0.6)

      w = weights
      w.preload([ p1, p2 ])

      query_count = 0
      callback = ->(_name, _start, _finish, _id, payload) {
        next if payload[:name].to_s =~ /SCHEMA|TRANSACTION|SAVEPOINT/i
        query_count += 1
      }
      ActiveSupport::Notifications.subscribed(callback, "sql.active_record") do
        expect(w.weight(p1)).to be_within(0.001).of(0.5)
        expect(w.weight(p2)).to be_within(0.001).of(0.6)
      end
      expect(query_count).to eq(0)
    end
  end

  describe "#top" do
    it "returns rows ordered by weight desc" do
      p1 = create(:person)
      p2 = create(:person)
      make_aw(p1, weight: 0.3)
      make_aw(p2, weight: 0.9)

      top = weights.top(2)
      expect(top.first.weight).to be_within(0.001).of(0.9)
    end
  end
end
