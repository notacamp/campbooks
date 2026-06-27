FactoryBot.define do
  factory :organization do
    workspace
    sequence(:name) { |n| "Organization #{n}" }
    domain { "org#{SecureRandom.hex(4)}.com" }
  end
end
