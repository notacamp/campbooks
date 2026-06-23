FactoryBot.define do
  factory :workflow do
    workspace
    sequence(:name) { |n| "Workflow #{n}" }
    trigger_type { "email_received" }
    enabled { true }

    trait :webhook do
      trigger_type { "webhook" }
    end
  end
end
