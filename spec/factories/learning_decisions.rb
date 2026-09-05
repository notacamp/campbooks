FactoryBot.define do
  factory :learning_decision do
    association :user
    association :workspace
    domain  { "email_skim" }
    label   { "archive" }
  end
end
