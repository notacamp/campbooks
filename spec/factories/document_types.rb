FactoryBot.define do
  factory :document_type do
    workspace
    sequence(:name) { |n| "document_type_#{n}" }
    category { "accounting" }
    color { "#3B82F6" }
  end
end
