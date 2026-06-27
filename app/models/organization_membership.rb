class OrganizationMembership < ApplicationRecord
  belongs_to :person
  belongs_to :organization
  enum :status, { active: 0, inactive: 1 }
  validates :person_id, uniqueness: { scope: :organization_id, message: :taken }
end
