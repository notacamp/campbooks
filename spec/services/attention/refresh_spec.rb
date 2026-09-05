# frozen_string_literal: true

require "rails_helper"

RSpec.describe Attention::Refresh do
  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace) }
  let(:account)   { create(:email_account, workspace: workspace) }
  let(:now)       { Time.zone.local(2026, 9, 5, 12, 0, 0) }

  before do
    create(:email_account_user, user: user, email_account: account, can_read: true)
  end

  def make_person(starred: false)
    person  = create(:person)
    contact = create(:contact, workspace: workspace, email_account: account, person: person,
                     sender_kind: 0, email_count: 2, starred_at: starred ? Time.current : nil)
    thread  = create(:email_thread, email_account: account)
    create(:email_message, email_account: account, contact: contact, email_thread: thread,
           from_address: contact.email, to_address: account.email_address, received_at: now - 5.days, status: :processed)
    person
  end

  def call(u = user)
    described_class.call(u, now: now)
  end

  describe "basic functionality" do
    it "writes Person rows with reasons and evidence" do
      person = make_person
      count = call

      aw = AttentionWeight.find_by(user: user, subject: person)
      expect(aw).not_to be_nil
      expect(aw.weight).to be_a(Float)
      expect(aw.confidence).to be_a(Float)
      expect(aw.reasons).to be_an(Array)
      expect(aw.evidence).to be_a(Hash)
      expect(aw.computed_at).to be_within(1.second).of(now)
      expect(count).to eq(1)
    end

    it "returns 0 for a user without a workspace" do
      user.update_columns(workspace_id: nil)
      expect(call).to eq(0)
    end
  end

  describe "organization rows" do
    it "inherits the best member's weight and prepends org_lead reason" do
      person = make_person
      org = create(:organization)
      create(:organization_membership, person: person, organization: org, status: :active)

      call

      org_aw = AttentionWeight.find_by(user: user, subject: org)
      expect(org_aw).not_to be_nil
      expect(org_aw.reasons.first["key"]).to eq("org_lead")
    end
  end

  describe "idempotence" do
    it "second run changes only computed_at, not the weight" do
      make_person
      call

      aw1 = AttentionWeight.for_user(user).first
      w1  = aw1.weight

      travel_to(now + 1.minute) do
        described_class.call(user, now: now + 1.minute)
      end

      aw2 = AttentionWeight.for_user(user).first.reload
      expect(aw2.weight).to be_within(0.001).of(w1)
    end
  end

  describe "pruning" do
    it "removes rows for counterparts that became ineligible" do
      person = make_person
      call

      expect(AttentionWeight.find_by(user: user, subject: person)).not_to be_nil

      # Make the person ineligible by reclassifying the contact as service
      Contact.where(person: person).update_all(sender_kind: 1)

      described_class.call(user, now: now)

      expect(AttentionWeight.find_by(user: user, subject: person)).to be_nil
    end
  end

  describe "error isolation" do
    it "logs and skips a person whose scoring raises, but writes other rows" do
      make_person
      good_person = make_person

      call_count = 0
      allow(Attention::Scorer).to receive(:score) do |facts|
        call_count += 1
        raise "scoring exploded" if call_count == 1
        Attention::Scorer.new(facts).score # call original for subsequent
      end

      expect(Rails.logger).to receive(:warn).with(/Attention::Refresh.*scoring exploded/).at_least(:once)

      count = call

      # At least the good person's row should be present
      expect(AttentionWeight.for_user(user).count).to be >= 1
    end
  end

  describe "return value" do
    it "returns the number of rows written (persons + orgs)" do
      p1 = make_person
      p2 = make_person
      org = create(:organization)
      create(:organization_membership, person: p1, organization: org, status: :active)

      count = call
      expect(count).to eq(3) # 2 persons + 1 org
    end
  end
end
