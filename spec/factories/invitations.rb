FactoryBot.define do
  factory :invitation do
    workspace
    association :invited_by, factory: :user
    sequence(:email) { |n| "invite-#{n}@example.com" }
    status { :pending }
    token { SecureRandom.urlsafe_base64(32) }
    expires_at { 7.days.from_now }

    trait :pending do
      status { :pending }
    end

    trait :accepted do
      status { :accepted }
      association :accepted_by, factory: :user
      accepted_at { Time.current }
    end

    trait :cancelled do
      status { :cancelled }
    end

    trait :expired do
      status { :pending }
      expires_at { 1.day.ago }
    end
  end
end
