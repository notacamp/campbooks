# frozen_string_literal: true

require "rails_helper"

RSpec.describe ScheduledEmail do
  describe "validations" do
    it "requires to_address" do
      email = build(:scheduled_email, to_address: nil)
      expect(email).not_to be_valid
      expect(email.errors[:to_address]).to be_present
    end

    it "requires subject" do
      email = build(:scheduled_email, subject: nil)
      expect(email).not_to be_valid
      expect(email.errors[:subject]).to be_present
    end

    it "requires body" do
      email = build(:scheduled_email, body: nil)
      expect(email).not_to be_valid
      expect(email.errors[:body]).to be_present
    end

    it "requires scheduled_at" do
      email = build(:scheduled_email, scheduled_at: nil)
      expect(email).not_to be_valid
      expect(email.errors[:scheduled_at]).to be_present
    end
  end

  describe "scopes" do
    let(:workspace) { create(:workspace) }
    let(:user) { create(:user, workspace: workspace) }
    let(:other_workspace) { create(:workspace) }
    let(:other_user) { create(:user, workspace: other_workspace) }
    let!(:my_email) { create(:scheduled_email, workspace: workspace, created_by: user) }
    let!(:other_email) { create(:scheduled_email, workspace: other_workspace, created_by: other_user) }

    it "accessible_to scopes to user's workspace" do
      expect(described_class.accessible_to(user)).to include(my_email)
      expect(described_class.accessible_to(user)).not_to include(other_email)
    end

    it "due returns pending items with past scheduled_at" do
      future = create(:scheduled_email, workspace: workspace, created_by: user, scheduled_at: 1.day.from_now)
      past = create(:scheduled_email, workspace: workspace, created_by: user, scheduled_at: 1.hour.ago)
      cancelled = create(:scheduled_email, workspace: workspace, created_by: user, scheduled_at: 1.hour.ago, status: :cancelled)

      due = described_class.accessible_to(user).due
      expect(due).to include(past)
      expect(due).not_to include(future)
      expect(due).not_to include(cancelled)
    end
  end

  describe "#recurring?" do
    it "is true when rrule is present" do
      email = build(:scheduled_email, rrule: "FREQ=DAILY")
      expect(email.recurring?).to be(true)
    end

    it "is false when rrule is nil" do
      email = build(:scheduled_email, rrule: nil)
      expect(email.recurring?).to be(false)
    end

    it "is false when rrule is blank" do
      email = build(:scheduled_email, rrule: "")
      expect(email.recurring?).to be(false)
    end
  end

  describe "#display_time" do
    it "returns next_occurrence_at when present" do
      email = build(:scheduled_email, scheduled_at: 2.days.from_now, next_occurrence_at: 1.day.from_now)
      expect(email.display_time).to eq(email.next_occurrence_at)
    end

    it "falls back to scheduled_at" do
      email = build(:scheduled_email, scheduled_at: 2.days.from_now, next_occurrence_at: nil)
      expect(email.display_time).to eq(email.scheduled_at)
    end
  end

  describe "#display_time on a recurring item" do
    it "uses next_occurrence_at once advanced" do
      email = build(:scheduled_email, :recurring, scheduled_at: 1.day.from_now, next_occurrence_at: 2.hours.from_now)
      expect(email.display_time).to eq(email.next_occurrence_at)
    end
  end
end
