FactoryBot.define do
  factory :device do
    user
    platform { :ios }
    sequence(:token) { |n| "device-token-#{n}" }
    last_active_at { Time.current }
  end
end
