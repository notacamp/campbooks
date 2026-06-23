FactoryBot.define do
  factory :calendar_account do
    workspace
    email_address { Faker::Internet.email }
    provider_account_id { Faker::Number.number(digits: 16).to_s }
    refresh_token { "1/#{SecureRandom.hex(32)}" }
    provider { :google }
    active { true }
  end
end
