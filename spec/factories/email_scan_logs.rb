FactoryBot.define do
  factory :email_scan_log do
    email_account
    status { :completed }
    started_at { Faker::Time.backward(days: 1) }
    completed_at { Time.current }
    emails_found { Faker::Number.between(from: 0, to: 10) }
    emails_processed { Faker::Number.between(from: 0, to: 5) }
    documents_created { Faker::Number.between(from: 0, to: 5) }
    errors_count { 0 }
  end
end
