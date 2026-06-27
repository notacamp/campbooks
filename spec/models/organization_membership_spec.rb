require "rails_helper"

RSpec.describe OrganizationMembership, type: :model do
  describe "associations" do
    it { is_expected.to belong_to(:person) }
    it { is_expected.to belong_to(:organization) }
  end

  describe "enums" do
    it "has active as default status" do
      membership = create(:organization_membership)
      expect(membership).to be_active
    end

    it "allows inactive status" do
      membership = create(:organization_membership, status: :inactive)
      expect(membership).to be_inactive
    end
  end

  describe "validations" do
    it "validates uniqueness of person scoped to organization" do
      person = create(:person)
      org = create(:organization)
      create(:organization_membership, person: person, organization: org)
      duplicate = build(:organization_membership, person: person, organization: org)
      expect(duplicate).not_to be_valid
    end
  end
end
