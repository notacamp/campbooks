FactoryBot.define do
  factory :scheduled_email do
    workspace
    email_account
    association :created_by, factory: :user
    to_address { "recipient@example.com" }
    subject { "Scheduled test email" }
    body { "<p>This is a scheduled email.</p>" }
    scheduled_at { 1.hour.from_now }
    status { :pending }

    trait :due do
      scheduled_at { 1.hour.ago }
    end

    trait :recurring do
      rrule { "FREQ=DAILY" }
    end

    trait :cancelled do
      status { :cancelled }
    end
  end
end
