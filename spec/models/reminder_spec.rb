require "rails_helper"

RSpec.describe Reminder do
  describe "enums" do
    it "defines the taxonomy and lifecycle" do
      expect(described_class.reminder_types).to include("payment_due", "delivery", "deadline", "other")
      expect(described_class.statuses).to include("pending", "confirmed", "dismissed", "snoozed")
    end
  end

  describe "scopes" do
    let(:workspace) { create(:workspace) }

    it "splits overdue and upcoming by due_at, pending only" do
      overdue   = create(:reminder, workspace: workspace, due_at: 2.days.ago)
      upcoming  = create(:reminder, workspace: workspace, due_at: 2.days.from_now)
      confirmed = create(:reminder, :confirmed, workspace: workspace, due_at: 1.day.ago)

      expect(described_class.overdue).to contain_exactly(overdue)
      expect(described_class.upcoming).to contain_exactly(upcoming)
      expect(described_class.overdue).not_to include(confirmed)
    end
  end

  describe ".accessible_to" do
    let(:workspace) { create(:workspace) }
    let(:user)      { create(:user, workspace: workspace) }

    it "returns none for a nil user" do
      expect(described_class.accessible_to(nil)).to be_empty
    end

    it "includes document-sourced reminders in the user's workspace" do
      mine  = create(:reminder, workspace: workspace, source: create(:document, workspace: workspace))
      other = create(:reminder, workspace: create(:workspace))

      result = described_class.accessible_to(user)
      expect(result).to include(mine)
      expect(result).not_to include(other)
    end

    it "gates email-sourced reminders on a readable account" do
      readable = create(:email_account, workspace: workspace)
      create(:email_account_user, user: user, email_account: readable, can_read: true)
      hidden   = create(:email_account, workspace: workspace)

      visible_reminder = create(:reminder, workspace: workspace, source: create(:email_message, email_account: readable))
      hidden_reminder  = create(:reminder, workspace: workspace, source: create(:email_message, email_account: hidden))

      result = described_class.accessible_to(user)
      expect(result).to include(visible_reminder)
      expect(result).not_to include(hidden_reminder)
    end
  end

  describe ".fingerprint_for" do
    it "is stable for the same inputs and varies by date" do
      a = described_class.fingerprint_for(source_type: "Document", source_id: 1, reminder_type: "payment_due", due_date: "2026-07-15")
      b = described_class.fingerprint_for(source_type: "Document", source_id: 1, reminder_type: "payment_due", due_date: "2026-07-15")
      c = described_class.fingerprint_for(source_type: "Document", source_id: 1, reminder_type: "payment_due", due_date: "2026-07-16")

      expect(a).to eq(b)
      expect(a).not_to eq(c)
    end
  end
end
