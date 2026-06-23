FactoryBot.define do
  factory :contact_email_alias do
    contact
    email { Faker::Internet.email }
  end
end
