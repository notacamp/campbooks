FactoryBot.define do
  factory :feed_item do
    association :user
    association :workspace
    association :subject, factory: :email_message
    kind        { "reply_owed" }
    sort_at     { Time.current }
    score       { 50 }
    dedupe_key  { SecureRandom.hex(8) }
  end
end
