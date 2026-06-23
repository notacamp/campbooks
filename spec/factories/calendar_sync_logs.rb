FactoryBot.define do
  factory :calendar_sync_log do
    calendar_account
    status { :completed }
    started_at { Faker::Time.backward(days: 1) }
    completed_at { Time.current }
    events_found { 0 }
    events_upserted { 0 }
  end
end
