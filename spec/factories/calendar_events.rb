FactoryBot.define do
  factory :calendar_event do
    calendar
    sequence(:provider_event_id) { |n| "evt_#{n}" }
    title { Faker::Lorem.sentence(word_count: 3) }
    start_at { 1.day.from_now.beginning_of_hour }
    end_at { 1.day.from_now.beginning_of_hour + 1.hour }
    all_day { false }
    status { :confirmed }
    is_organizer { false }
    outbound_pending { false }
    attendees { [] }

    trait :cancelled do
      status { :cancelled }
    end

    trait :tentative do
      status { :tentative }
    end

    trait :all_day do
      all_day { true }
      start_at { Date.tomorrow.to_time.utc }
      end_at { Date.tomorrow.to_time.utc + 1.day }
    end

    trait :recurring do
      recurring_event_provider_id { "series_#{SecureRandom.hex(8)}" }
    end

    trait :outbound_pending do
      outbound_pending { true }
      sequence(:provider_event_id) { |n| "local-#{SecureRandom.uuid}-#{n}" }
    end
  end
end
