# frozen_string_literal: true

FactoryBot.define do
  factory :reconciliation do
    workspace
    association :created_by, factory: :user
    association :statement_document, factory: :document

    currency { "EUR" }
    status { :pending }
    export_status { :export_none }
    integrity_warning { false }

    trait :parsing do
      status { :parsing }
    end

    trait :ready do
      status { :ready }
    end

    trait :failed do
      status { :failed }
      parse_error { "Could not parse file." }
    end

    trait :with_period do
      period_start { Date.new(2024, 1, 1) }
      period_end   { Date.new(2024, 1, 31) }
    end

    trait :with_bank do
      bank_name { "Millennium BCP" }
    end

    trait :with_integrity_warning do
      integrity_warning { true }
      integrity_warning_message { "Balance check failed: expected 1000.00 but statement shows 990.00." }
    end
  end
end
