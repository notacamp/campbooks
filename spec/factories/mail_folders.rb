FactoryBot.define do
  factory :mail_folder do
    workspace
    sequence(:name) { |n| "Folder #{n}" }
    position { 0 }
  end
end
