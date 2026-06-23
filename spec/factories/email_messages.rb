FactoryBot.define do
  factory :email_message do
    email_account
    email_scan_log
    provider_message_id { Faker::Number.number(digits: 16).to_s }
    provider_folder_id { Faker::Number.number(digits: 16).to_s }
    from_address { Faker::Internet.email }
    subject { Faker::Lorem.sentence }
    received_at { Faker::Time.backward(days: 7) }
    status { :fetched }
  end
end
