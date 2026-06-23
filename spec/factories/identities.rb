FactoryBot.define do
  factory :identity do
    user
    provider { "google" }
    sequence(:uid) { |n| "uid-#{n}" }
    email { Faker::Internet.email }
  end
end
