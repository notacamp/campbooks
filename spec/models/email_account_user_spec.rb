require "rails_helper"

RSpec.describe EmailAccountUser, type: :model do
  describe "#role" do
    it "derives the role from the permission flags" do
      expect(build(:email_account_user, :viewer).role).to eq("viewer")
      expect(build(:email_account_user, :collaborator).role).to eq("collaborator")
      expect(build(:email_account_user, :manager).role).to eq("manager")
    end

    it "reports the owner row as 'owner' regardless of flags" do
      expect(build(:email_account_user, :owner).role).to eq("owner")
    end
  end

  describe "#role=" do
    it "sets all three flags for a known role" do
      entry = build(:email_account_user)
      entry.role = "manager"

      expect(entry.can_read).to be(true)
      expect(entry.can_send).to be(true)
      expect(entry.can_manage).to be(true)
    end

    it "ignores an unknown role rather than clearing access" do
      entry = build(:email_account_user, :collaborator)
      entry.role = "superadmin"

      expect(entry.role).to eq("collaborator")
    end
  end
end
