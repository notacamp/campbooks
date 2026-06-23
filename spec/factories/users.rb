FactoryBot.define do
  factory :user do
    workspace
    name { Faker::Name.first_name }
    email_address { Faker::Internet.email }
    password { "password123" }
    password_confirmation { "password123" }
    # Factory users hold a real, user-chosen password — distinguishes them from
    # OAuth-only users for Auth::OauthSignIn / sign-in-method counts.
    password_set_by_user { true }
  end
end
