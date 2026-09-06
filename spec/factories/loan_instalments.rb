# frozen_string_literal: true

FactoryBot.define do
  factory :loan_instalment do
    loan
    sequence(:number) { |n| n }
    expected_on  { loan.first_instalment_on >> (number - 1) }
    amount_cents { loan.instalment_cents }
    status       { :expected }

    trait :paid do
      status { :paid }
    end

    trait :missed do
      status { :missed }
    end

    trait :unverified do
      status { :unverified }
    end

    trait :with_transaction do
      association :bank_transaction
      status { :paid }
    end

    trait :rate_reset do
      status               { :paid }
      previous_amount_cents { amount_cents - 1_200 }
    end
  end
end
