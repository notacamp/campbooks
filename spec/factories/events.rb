FactoryBot.define do
  factory :event do
    workspace
    name { "document.processed" }
    payload { {} }
    occurred_at { Time.current }
  end
end
