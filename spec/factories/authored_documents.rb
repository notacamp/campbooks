FactoryBot.define do
  factory :authored_document do
    workspace
    title { Faker::Lorem.sentence(word_count: 4) }
    html_content { "<h2>#{Faker::Lorem.sentence}</h2><p>#{Faker::Lorem.paragraph}</p>" }
    association :author, factory: :user
  end
end
