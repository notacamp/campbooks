FactoryBot.define do
  factory :email_folder do
    email_account
    sequence(:provider_folder_id) { |n| "folder_#{n}" }
    name { "Inbox" }
    position { 1 }
  end
end
