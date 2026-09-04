# frozen_string_literal: true

require "rails_helper"

RSpec.describe PeopleStanding do
  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace) }
  let(:account)   { create(:email_account, workspace: workspace) }

  before { create(:email_account_user, user: user, email_account: account, can_read: true) }

  def make_standing(counterpart:, score: 0.5, needs_you: false, last_activity_at: 1.day.ago,
                    standing_kind: "last_exchange", text: "Last exchange yesterday.")
    PeopleStanding.create!(
      workspace: workspace,
      user: user,
      counterpart: counterpart,
      needs_you: needs_you,
      standing_kind: standing_kind,
      text: text,
      score: score,
      strength: 1.0,
      last_activity_at: last_activity_at,
      name: counterpart.respond_to?(:display_name) ? counterpart.display_name : counterpart.name,
      avatar_initial: counterpart.respond_to?(:name) ? counterpart.name[0].upcase : "?",
      refreshed_at: Time.current
    )
  end

  describe ".ranked scope" do
    it "orders by score descending" do
      person_a = create(:person, workspace: workspace, name: "Alpha")
      person_b = create(:person, workspace: workspace, name: "Beta")
      row_a = make_standing(counterpart: person_a, score: 0.2)
      row_b = make_standing(counterpart: person_b, score: 0.9)

      expect(described_class.for_user(user).ranked.to_a).to eq([ row_b, row_a ])
    end

    it "on a tie, persons sort before organizations (Person > Organization lexicographically)" do
      person = create(:person, workspace: workspace, name: "Rui")
      org    = create(:organization, workspace: workspace, name: "Rui Corp")

      person_row = make_standing(counterpart: person, score: 1.0, last_activity_at: 2.days.ago)
      org_row    = make_standing(counterpart: org,    score: 1.0, last_activity_at: 2.days.ago)

      ranked = described_class.for_user(user).ranked.to_a
      expect(ranked.index(person_row)).to be < ranked.index(org_row)
    end

    it "on a further tie, the livelier row comes first" do
      person_a = create(:person, workspace: workspace, name: "Aina")
      person_b = create(:person, workspace: workspace, name: "Bea")
      row_a = make_standing(counterpart: person_a, score: 1.0, last_activity_at: 1.hour.ago)
      row_b = make_standing(counterpart: person_b, score: 1.0, last_activity_at: 7.days.ago)

      expect(described_class.for_user(user).ranked.first).to eq(row_a)
    end
  end

  describe ".search" do
    it "matches by name (case-insensitive)" do
      person = create(:person, workspace: workspace, name: "Sofia Martins")
      other  = create(:person, workspace: workspace, name: "Ana Reis")
      make_standing(counterpart: person, text: "Last exchange.")
      make_standing(counterpart: other,  text: "Last exchange.")

      results = described_class.for_user(user).search("sofia")
      expect(results.pluck(:name)).to include("Sofia Martins")
      expect(results.pluck(:name)).not_to include("Ana Reis")
    end

    it "matches by subtitle" do
      person = create(:person, workspace: workspace, name: "Rui")
      PeopleStanding.create!(
        workspace: workspace, user: user, counterpart: person,
        needs_you: false, standing_kind: "last_exchange", text: "t",
        score: 0.0, strength: 0.0, name: "Rui",
        subtitle: "Cloudhost Corp",
        refreshed_at: Time.current
      )

      expect(described_class.for_user(user).search("Cloudhost").count).to eq(1)
    end

    it "matches by avatar_email" do
      person = create(:person, workspace: workspace, name: "Nadia")
      PeopleStanding.create!(
        workspace: workspace, user: user, counterpart: person,
        needs_you: false, standing_kind: "last_exchange", text: "t",
        score: 0.0, strength: 0.0, name: "Nadia",
        avatar_email: "nadia@brightloop.example",
        refreshed_at: Time.current
      )

      expect(described_class.for_user(user).search("brightloop").count).to eq(1)
      expect(described_class.for_user(user).search("unknown").count).to eq(0)
    end
  end

  describe "#to_counterpart" do
    it "builds a People::Counterpart with no additional queries" do
      person = create(:person, workspace: workspace, name: "Ines")
      row = PeopleStanding.create!(
        workspace: workspace, user: user, counterpart: person,
        needs_you: true, standing_kind: "attention",
        text: nil, verb: "reply", subject: "Q3 deck", wait_days: 2,
        score: 1.5, strength: 2.0,
        last_activity_at: 2.days.ago, name: "Ines",
        subtitle: "Brightloop", avatar_email: "ines@brightloop.example",
        overdue_days: 2, refreshed_at: Time.current
      )

      query_count = 0
      sub = ActiveSupport::Notifications.subscribe("sql.active_record") do |*, payload|
        query_count += 1 unless payload[:cached] || %w[SCHEMA TRANSACTION].include?(payload[:name])
      end

      cp = row.to_counterpart

      ActiveSupport::Notifications.unsubscribe(sub)
      expect(query_count).to eq(0), "to_counterpart fired #{query_count} queries"

      expect(cp).to be_a(People::Counterpart)
      expect(cp.id).to eq(person.id)
      expect(cp.person?).to be true
      expect(cp.name).to eq("Ines")
      expect(cp.subtitle).to eq("Brightloop")
      expect(cp.avatar_email).to eq("ines@brightloop.example")
      expect(cp.needs_you?).to be true
      expect(cp.standing.kind).to eq(:attention)
      expect(cp.standing.verb).to eq(:reply)
      expect(cp.standing.subject).to eq("Q3 deck")
      expect(cp.score.value).to eq(1.5)
      expect(cp.score.strength).to eq(2.0)
    end
  end
end
