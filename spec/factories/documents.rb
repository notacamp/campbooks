FactoryBot.define do
  factory :document do
    workspace
    document_type { :expense_invoice }
    ai_status { :pending }
    review_status { :pending }
    source { :manual_upload }
    vendor_name { Faker::Company.name }
    vendor_nif { Faker::Number.number(digits: 9).to_s }
    document_date { Faker::Date.backward(days: 30) }
    invoice_number { "FT#{Date.today.year}/#{Faker::Number.number(digits: 4)}" }
    amount_cents { Faker::Number.between(from: 1000, to: 100_000) }
    currency { "EUR" }
    tax_amount_cents { (amount_cents * 0.23).to_i }
    tax_rate { 23.0 }

    after(:build) do |document|
      document.original_file.attach(
        io: StringIO.new("fake pdf content"),
        filename: "invoice.pdf",
        content_type: "application/pdf"
      )
    end

    # AI finished, awaiting human sign-off — the Skim / review-queue state.
    trait :in_review do
      ai_status { :completed }
      review_status { :pending }
    end

    trait :approved do
      ai_status { :completed }
      review_status { :approved }
    end

    trait :rejected do
      ai_status { :completed }
      review_status { :rejected }
    end

    # AI hard-failure (adapter/parse/config error) — the "needs attention" lane.
    trait :ai_failed do
      ai_status { :failed }
      ai_error { "AI analysis error" }
    end

    trait :revenue_invoice do
      document_type { :revenue_invoice }
      vendor_name { nil }
      vendor_nif { nil }
      client_name { Faker::Company.name }
      client_nif { Faker::Number.number(digits: 9).to_s }
    end

    trait :bank_statement do
      document_type { :bank_statement }
      vendor_name { nil }
      vendor_nif { nil }
      invoice_number { nil }
      amount_cents { nil }
      tax_amount_cents { nil }
      tax_rate { nil }
      bank_name { [ "Millennium BCP", "Caixa Geral de Depósitos", "Novo Banco", "BPI" ].sample }
      account_number { Faker::Bank.iban(country_code: "pt") }
      period_start { Date.current.beginning_of_month }
      period_end { Date.current.end_of_month }
      opening_balance_cents { Faker::Number.between(from: 100_000, to: 1_000_000) }
      closing_balance_cents { Faker::Number.between(from: 100_000, to: 1_000_000) }
    end

    trait :receipt do
      document_type { :receipt }
      invoice_number { nil }
      tax_amount_cents { nil }
      tax_rate { nil }
      receipt_number { "RC#{Date.today.year}/#{Faker::Number.number(digits: 4)}" }
      payment_method { %w[cash card transfer mbway multibanco].sample }
    end

    trait :other do
      document_type { :other }
      vendor_nif { nil }
      invoice_number { nil }
      amount_cents { nil }
      tax_amount_cents { nil }
      tax_rate { nil }
    end
  end
end
