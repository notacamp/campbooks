FactoryBot.define do
  factory :notification do
    user
    title { "Something happened" }
    category { :activity }
    priority { :activity }
    count { 1 }
  end
end
