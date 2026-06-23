FactoryBot.define do
  factory :calendar do
    calendar_account
    sequence(:provider_calendar_id) { |n| "cal_#{n}@group.calendar.google.com" }
    name { Faker::Lorem.words(number: 2).join(" ").capitalize }
    is_primary { false }
    is_writable { true }
    syncing { true }
    color { "#3b82f6" }

    trait :primary do
      is_primary { true }
      sequence(:provider_calendar_id) { |n| "primary_#{n}@gmail.com" }
    end

    trait :read_only do
      is_writable { false }
    end

    trait :not_syncing do
      syncing { false }
    end
  end
end
