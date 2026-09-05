# frozen_string_literal: true

require "rails_helper"

RSpec.describe AttentionWeight do
  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace) }
  let(:person)    { create(:person) }

  def build_weight(**overrides)
    build(:attention_weight, user: user, workspace: workspace, subject: person, **overrides)
  end

  describe "validations" do
    it "is valid with valid attributes" do
      aw = build_weight(weight: 0.5, confidence: 0.7, computed_at: Time.current)
      expect(aw).to be_valid
    end

    it "requires subject_type to be Person or Organization" do
      aw = build_weight
      aw.subject_type = "EmailMessage"
      expect(aw).not_to be_valid
      expect(aw.errors[:subject_type]).to be_present
    end

    it "rejects weight outside 0..1" do
      expect(build_weight(weight: -0.1)).not_to be_valid
      expect(build_weight(weight: 1.01)).not_to be_valid
    end

    it "rejects confidence outside 0..1" do
      expect(build_weight(confidence: -0.1)).not_to be_valid
      expect(build_weight(confidence: 1.01)).not_to be_valid
    end

    it "accepts weight and confidence at the boundaries 0.0 and 1.0" do
      expect(build_weight(weight: 0.0, confidence: 0.0)).to be_valid
      expect(build_weight(weight: 1.0, confidence: 1.0)).to be_valid
    end
  end

  describe "#reason_values" do
    it "returns Attention::Reason objects from the stored reasons array" do
      aw = build_weight(reasons: [
        { "key" => "replies_fast", "params" => { "hours" => 3 } },
        { "key" => "meetings", "params" => { "count" => 2 } }
      ])
      expect(aw.reason_values).to all(be_a(Attention::Reason))
      expect(aw.reason_values.map(&:key)).to eq(%w[replies_fast meetings])
    end

    it "returns an empty array when reasons is empty" do
      aw = build_weight(reasons: [])
      expect(aw.reason_values).to eq([])
    end
  end

  describe "#person? / #organization?" do
    it "identifies the subject type correctly" do
      aw = build_weight
      aw.subject_type = "Person"
      expect(aw.person?).to be true
      expect(aw.organization?).to be false

      aw.subject_type = "Organization"
      expect(aw.person?).to be false
      expect(aw.organization?).to be true
    end
  end

  describe "scopes" do
    it ".ranked orders by weight desc then confidence desc" do
      t = Time.current
      high  = create(:attention_weight, user: user, workspace: workspace, subject: create(:person), weight: 0.9, confidence: 0.8, computed_at: t)
      low   = create(:attention_weight, user: user, workspace: workspace, subject: create(:person), weight: 0.2, confidence: 0.5, computed_at: t)
      mid   = create(:attention_weight, user: user, workspace: workspace, subject: create(:person), weight: 0.6, confidence: 0.7, computed_at: t)
      expect(AttentionWeight.for_user(user).ranked.map(&:id)).to eq([ high.id, mid.id, low.id ])
    end
  end
end
