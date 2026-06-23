FactoryBot.define do
  factory :email_thread do
    email_account
    subject { Faker::Lorem.sentence }
  end
end
