require "rails_helper"
RSpec.describe OrganizationMembership, type: :model do
  it { is_expected.to belong_to(:person) }
  it { is_expected.to belong_to(:organization) }
  it "defaults to active" do
    expect(create(:organization_membership)).to be_active
  end
  it "validates person+org uniqueness" do
    p = create(:person); o = create(:organization)
    create(:organization_membership, person: p, organization: o)
    expect(build(:organization_membership, person: p, organization: o)).not_to be_valid
  end
end
