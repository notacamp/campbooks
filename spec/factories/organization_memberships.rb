FactoryBot.define do
  factory :organization_membership do
    person
    organization
    status { :active }
  end
end
