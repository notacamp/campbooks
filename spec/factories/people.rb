FactoryBot.define do
  factory :person do
    name { Faker::Name.name }
    organization { Faker::Company.name }
    relationship_type { "vendor" }
    context_summary { Faker::Lorem.paragraph }
    communication_patterns do
      { "typical_topics" => [ "invoicing" ], "tone" => "formal", "urgency_level" => "medium", "primary_role" => "accounts payable" }
    end
    analyzed_at { Time.current }
  end
end
