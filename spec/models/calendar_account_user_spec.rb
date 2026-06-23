require "rails_helper"

RSpec.describe CalendarAccountUser, type: :model do
  describe "ROLE_FLAGS" do
    it "defines flags for viewer, editor, and manager" do
      expect(CalendarAccountUser::ROLE_FLAGS.keys).to contain_exactly("viewer", "editor", "manager")
    end

    it "viewer has only can_read" do
      flags = CalendarAccountUser::ROLE_FLAGS["viewer"]
      expect(flags).to eq(can_read: true, can_write: false, can_manage: false)
    end

    it "editor has can_read and can_write" do
      flags = CalendarAccountUser::ROLE_FLAGS["editor"]
      expect(flags).to eq(can_read: true, can_write: true, can_manage: false)
    end

    it "manager has all three flags" do
      flags = CalendarAccountUser::ROLE_FLAGS["manager"]
      expect(flags).to eq(can_read: true, can_write: true, can_manage: true)
    end
  end

  describe "#role" do
    it "derives the role from the permission flags" do
      expect(build(:calendar_account_user, :viewer).role).to eq("viewer")
      expect(build(:calendar_account_user, :editor).role).to eq("editor")
      expect(build(:calendar_account_user, :manager).role).to eq("manager")
    end

    it "reports the owner row as 'owner' regardless of flags" do
      expect(build(:calendar_account_user, :owner).role).to eq("owner")
    end
  end

  describe "#role=" do
    it "sets all three flags for 'manager'" do
      entry = build(:calendar_account_user)
      entry.role = "manager"

      expect(entry.can_read).to be(true)
      expect(entry.can_write).to be(true)
      expect(entry.can_manage).to be(true)
    end

    it "sets can_read and can_write but not can_manage for 'editor'" do
      entry = build(:calendar_account_user)
      entry.role = "editor"

      expect(entry.can_read).to be(true)
      expect(entry.can_write).to be(true)
      expect(entry.can_manage).to be(false)
    end

    it "sets only can_read for 'viewer'" do
      entry = build(:calendar_account_user, :manager)
      entry.role = "viewer"

      expect(entry.can_read).to be(true)
      expect(entry.can_write).to be(false)
      expect(entry.can_manage).to be(false)
    end

    it "ignores an unknown role rather than clearing access" do
      entry = build(:calendar_account_user, :editor)
      entry.role = "superadmin"

      expect(entry.role).to eq("editor")
    end
  end
end
