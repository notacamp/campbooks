require "rails_helper"

RSpec.describe Invitation, type: :model do
  describe "associations" do
    it { should belong_to(:workspace) }
    it { should belong_to(:invited_by).class_name("User") }
    it { should belong_to(:accepted_by).class_name("User").optional }
  end

  describe "enums" do
    it { should define_enum_for(:status).with_values(pending: 0, accepted: 1, expired: 2, cancelled: 3) }
  end

  describe "validations" do
    it { should validate_presence_of(:email) }

    it "validates email format" do
      invitation = build(:invitation, email: "invalid")
      expect(invitation).not_to be_valid
      expect(invitation.errors[:email]).to include("is invalid")
    end

    it "validates email is not already a member" do
      user = create(:user, workspace: create(:workspace))
      invitation = build(:invitation, workspace: user.workspace, email: user.email_address)
      expect(invitation).not_to be_valid
      expect(invitation.errors[:email]).to include("is already a member of this workspace")
    end

    it "validates no duplicate pending invitation" do
      org = create(:workspace)
      create(:invitation, workspace: org, email: "test@example.com", status: :pending)
      dup = build(:invitation, workspace: org, email: "test@example.com")
      expect(dup).not_to be_valid
      expect(dup.errors[:email]).to include("has already been invited")
    end
  end

  describe "scopes" do
    describe ".active" do
      it "includes pending invitations that have not expired" do
        active = create(:invitation, status: :pending, expires_at: 1.day.from_now)
        expect(Invitation.active).to include(active)
      end

      it "excludes expired pending invitations" do
        expired = create(:invitation, :expired)
        expect(Invitation.active).not_to include(expired)
      end

      it "excludes accepted invitations" do
        accepted = create(:invitation, :accepted)
        expect(Invitation.active).not_to include(accepted)
      end
    end
  end

  describe "#expired?" do
    it "returns true for pending invitation past expiry" do
      invitation = build(:invitation, status: :pending, expires_at: 1.day.ago)
      expect(invitation).to be_expired
    end

    it "returns false for pending invitation before expiry" do
      invitation = build(:invitation, status: :pending, expires_at: 1.day.from_now)
      expect(invitation).not_to be_expired
    end

    it "returns false for non-pending invitations" do
      invitation = build(:invitation, status: :accepted, expires_at: 1.day.ago)
      expect(invitation).not_to be_expired
    end
  end

  describe "#accept!" do
    it "updates user's workspace and marks invitation as accepted" do
      org = create(:workspace)
      inviter = create(:user, workspace: org)
      user = create(:user)
      invitation = create(:invitation, workspace: create(:workspace), invited_by: inviter)

      invitation.accept!(user)

      user.reload
      expect(user.workspace).to eq(invitation.workspace)
      expect(invitation).to be_accepted
      expect(invitation.accepted_by).to eq(user)
      expect(invitation.accepted_at).to be_present
    end
  end

  describe "#cancel!" do
    it "marks invitation as cancelled" do
      invitation = create(:invitation)
      invitation.cancel!
      expect(invitation).to be_cancelled
    end
  end

  describe "#resend!" do
    it "regenerates token and resets to pending" do
      invitation = create(:invitation, :cancelled)
      old_token = invitation.token

      invitation.resend!

      expect(invitation.token).not_to eq(old_token)
      expect(invitation).to be_pending
      expect(invitation.expires_at).to be > 6.days.from_now
    end
  end
end
