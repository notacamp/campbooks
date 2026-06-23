FactoryBot.define do
  factory :contact do
    email { Faker::Internet.email }
    email_count { 0 }
    # Contact belongs_to :workspace (required). Follow the account's workspace when
    # one is given, otherwise build a standalone workspace.
    workspace { email_account&.workspace || association(:workspace) }

    trait :analyzed do
      name { Faker::Name.name }
      organization { Faker::Company.name }
      relationship_type { "vendor" }
      context_summary { Faker::Lorem.paragraph }
      communication_patterns do
        { "typical_topics" => [ "invoicing" ], "tone" => "formal", "urgency_level" => "medium", "primary_role" => "accounts payable" }
      end
      analyzed_at { Time.current }
    end

    trait :with_emails do
      transient do
        messages_count { 5 }
      end

      after(:create) do |contact, evaluator|
        evaluator.messages_count.times do
          create(:email_message, contact: contact, from_address: contact.email)
        end
        contact.update_columns(
          email_count: evaluator.messages_count,
          last_email_at: Time.current
        )
      end
    end

    trait :global do
      email_account { nil }
    end
  end
end
