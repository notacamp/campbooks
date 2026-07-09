# frozen_string_literal: true

FactoryBot.define do
  factory :transaction_match do
    bank_transaction
    document
    status     { :suggested }
    matched_by { :heuristic }
    confidence { 0.8 }
    match_reasons { { amount: true } }
  end
end
