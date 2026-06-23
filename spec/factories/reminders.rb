FactoryBot.define do
  factory :reminder do
    workspace
    association :source, factory: :document
    reminder_type { :payment_due }
    title { "Pay invoice" }
    due_at { 3.days.from_now }
    all_day { true }
    status { :pending }
    confidence { 0.9 }

    trait :from_email do
      association :source, factory: :email_message
    end

    trait :overdue do
      due_at { 2.days.ago }
    end

    trait :confirmed do
      status { :confirmed }
    end

    trait :snoozed do
      status { :snoozed }
      snoozed_until { 1.week.from_now }
    end
  end
end
