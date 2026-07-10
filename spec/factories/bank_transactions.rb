# frozen_string_literal: true

FactoryBot.define do
  factory :bank_transaction do
    reconciliation
    workspace { reconciliation.workspace }
    sequence(:position) { |n| n }
    booked_on   { Date.new(2024, 1, 15) }
    description { "Payment to #{Faker::Company.name}" }
    amount_cents { -5000 }   # debit by default
    currency     { "EUR" }
    raw_data     { {} }
    status       { :unmatched }

    trait :credit do
      amount_cents { 10_000 }
      description  { "Invoice receipt from #{Faker::Company.name}" }
    end

    trait :matched do
      status { :matched }
    end

    trait :excluded do
      status { :excluded }
      exclusion_reason { "Internal transfer" }
    end

    trait :with_counterparty do
      counterparty { Faker::Company.name }
    end

    trait :with_balance do
      balance_after_cents { 50_000 }
    end
  end
end
