FactoryBot.define do
  factory :email_account do
    workspace
    email_address { Faker::Internet.email }
    provider_account_id { Faker::Number.number(digits: 16).to_s }
    refresh_token { "1000.#{SecureRandom.hex(32)}" }
    active { true }
  end
end
