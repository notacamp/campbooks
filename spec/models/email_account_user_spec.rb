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

  # ── From EmailAccountUserTest (Minitest migration) ───────────────────────────

  describe "role ladder (extended coverage)" do
    before do
      @workspace = create(:workspace)
      @account   = create(:email_account, workspace: @workspace)
      @user      = create(:user, workspace: @workspace)
    end

    it "each assignable role maps to its exact flag bundle" do
      entry = create(:email_account_user, user: @user, email_account: @account)

      entry.role = "viewer"
      expect(entry).to be_can_read
      expect(entry).not_to be_can_send
      expect(entry).not_to be_can_manage

      entry.role = "collaborator"
      expect(entry).to be_can_read
      expect(entry).to be_can_send
      expect(entry).not_to be_can_manage

      entry.role = "manager"
      expect(entry).to be_can_read
      expect(entry).to be_can_send
      expect(entry).to be_can_manage
    end

    it "role is derived from flags, with owner taking precedence" do
      expect(create(:email_account_user, :viewer, user: @user, email_account: @account).role).to eq("viewer")
      expect(build(:email_account_user, :collaborator).role).to eq("collaborator")
      expect(build(:email_account_user, :manager).role).to eq("manager")
      expect(build(:email_account_user, :owner).role).to eq("owner")
    end

    it "an unknown role name changes nothing" do
      entry = create(:email_account_user, :viewer, user: @user, email_account: @account)

      entry.role = "superadmin"

      expect(entry).to be_can_read
      expect(entry).not_to be_can_send
      expect(entry).not_to be_can_manage
      expect(entry).not_to be_owner
    end

    it "a user can only be granted access to an account once" do
      create(:email_account_user, :viewer, user: @user, email_account: @account)
      duplicate = build(:email_account_user, :manager, user: @user, email_account: @account)

      expect(duplicate).not_to be_valid
    end
  end
end
