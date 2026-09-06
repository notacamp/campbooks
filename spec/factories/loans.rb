# frozen_string_literal: true

FactoryBot.define do
  factory :loan do
    workspace
    association :created_by, factory: :user
    lender              { "Test Bank" }
    source_counterparty { "TEST BANK" }
    principal_cents     { 2_400_000 }
    currency            { "EUR" }
    instalment_cents    { 40_000 }
    first_instalment_on { Date.new(2024, 1, 5) }
    term_months         { 60 }
    rate_note           { nil }
    status              { :active }

    trait :with_schedule do
      after(:create) { |loan| Loans::Schedule.build!(loan) }
    end

    trait :closed do
      status { :closed }
    end
  end
end
