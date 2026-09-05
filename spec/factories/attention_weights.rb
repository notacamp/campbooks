FactoryBot.define do
  factory :attention_weight do
    association :user
    association :workspace
    association :subject, factory: :person

    subject_type { "Person" }
    weight       { 0.5 }
    confidence   { 0.5 }
    raw_score    { 1.0 }
    reasons      { [] }
    evidence     { {} }
    computed_at  { Time.current }
  end
end
