FactoryBot.define do
  factory :connection do
    workspace
    sequence(:name) { |n| "Connection #{n}" }
    base_url { "https://api.example.com" }
    auth_type { "none" }

    trait :bearer do
      auth_type { "bearer" }
      auth_secret { "tok_secret" }
    end

    trait :header do
      auth_type { "header" }
      auth_header_name { "X-Api-Key" }
      auth_secret { "key_secret" }
    end

    trait :basic do
      auth_type { "basic" }
      auth_username { "user" }
      auth_secret { "pass" }
    end
  end
end
