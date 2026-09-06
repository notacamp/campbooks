# frozen_string_literal: true

# Previews for the Money surface's new evidence-based components.
# Uses lightweight in-memory structs and stub objects to avoid DB queries.
# NOTE: Statements preview is skipped because it requires persisted Reconciliation
# records with bank transactions; use the seeds (demo workspace) to preview it live.
class MoneyComponentsPreview < ViewComponent::Preview
  # Scout read with a ready statement — needs_invoice + review clauses.
  # @label Read (with statement)
  def read_with_statement
    read = stub_read(
      any_statements?: true,
      statement:       stub_statement("January", "Millennium BCP"),
      statement_label: "January",
      bank_name:       "Millennium BCP",
      lines_total:     18,
      lines_explained: 14,
      review_count:    2,
      needs_invoice_count: 3,
      needs_invoice_cents: 48_600,
      partial_count:   1,
      nif_count:       0,
      missing_count:   2,
      missing_cents:   180_000,
      missing_cents_by_currency: { "EUR" => 180_000 },
      primary_currency: "EUR",
      explained_pct:   78,
      requested_count: 0
    )
    render Campbooks::Money::Read.new(read: read, class: "max-w-[700px]")
  end

  # Scout read with no statements yet.
  # @label Read (no statements)
  def read_no_statements
    read = stub_read(any_statements?: false, statement: nil, statement_label: nil,
                     bank_name: nil, lines_total: 0, lines_explained: 0,
                     review_count: 0, needs_invoice_count: 0, needs_invoice_cents: 0,
                     partial_count: 0, nif_count: 0, missing_count: 0,
                     missing_cents: 0, missing_cents_by_currency: {}, primary_currency: "EUR",
                     explained_pct: 0, requested_count: 0)
    render Campbooks::Money::Read.new(read: read, class: "max-w-[700px]")
  end

  # Scout read when every line is explained and nothing is missing.
  # @label Read (all explained)
  def read_all_explained
    read = stub_read(
      any_statements?: true,
      statement:       stub_statement("August", "Novo Banco"),
      statement_label: "August",
      bank_name:       "Novo Banco",
      lines_total:     22,
      lines_explained: 22,
      review_count:    0,
      needs_invoice_count: 0,
      needs_invoice_cents: 0,
      partial_count:   0,
      nif_count:       0,
      missing_count:   0,
      missing_cents:   0,
      missing_cents_by_currency: {},
      primary_currency: "EUR",
      explained_pct:   100,
      requested_count: 0
    )
    render Campbooks::Money::Read.new(read: read, class: "max-w-[700px]")
  end

  # Summary stats strip.
  # @label Strip
  def strip
    read = stub_read(
      any_statements?: true,
      statement:       stub_statement("January", "Millennium BCP"),
      statement_label: "January",
      bank_name:       "Millennium BCP",
      lines_total:     18,
      lines_explained: 14,
      review_count:    2,
      needs_invoice_count: 3,
      needs_invoice_cents: 48_600,
      partial_count:   0,
      nif_count:       0,
      missing_count:   2,
      missing_cents:   180_000,
      missing_cents_by_currency: { "EUR" => 180_000 },
      primary_currency: "EUR",
      explained_pct:   78,
      requested_count: 0
    )
    render Campbooks::Money::Strip.new(read: read)
  end

  # Needs-you section — no invoice kind (resolve chip).
  # @label NeedsYou (no invoice)
  def needs_you_no_invoice
    txn = stub_txn(counterparty: "EDP Comercial", amount_cents: -9_640, booked_on: Date.new(2024, 1, 8))
    item = Money::NeedsYouItem.new(
      kind: :no_invoice, transaction: txn, match: nil, group: nil,
      title: "No invoice for this payment",
      meta: [ "EDP Comercial", "€96.40", "Jan 8, 2024" ],
      actions: [ :resolve ]
    )
    render Campbooks::Money::NeedsYou.new(items: [ item ], overflow: 0, statement: stub_statement("January", "Millennium BCP"))
  end

  # Needs-you section — review kind (confirm/change buttons).
  # @label NeedsYou (review)
  def needs_you_review
    txn = stub_txn(counterparty: "Vodafone PT", amount_cents: -24_800, booked_on: Date.new(2024, 1, 15))
    doc = OpenStruct.new(invoice_number: "FT2024/0021", entity_display_name: "Vodafone PT")
    match = OpenStruct.new(id: 42, confidence: 0.87, document: doc, suggested?: true)
    item = Money::NeedsYouItem.new(
      kind: :review, transaction: txn, match: match, group: nil,
      title: "Review this match",
      meta: [ "Vodafone PT", "€248.00", "Invoice #FT2024/0021", "87% likely" ],
      actions: [ :change, :confirm ]
    )
    render Campbooks::Money::NeedsYou.new(items: [ item ], overflow: 0, statement: stub_statement("January", "Millennium BCP"))
  end

  # Needs-you section — partial kind.
  # @label NeedsYou (partial)
  def needs_you_partial
    item = Money::NeedsYouItem.new(
      kind: :partial, transaction: nil, match: nil, group: nil,
      title: "Galp Frota is only part paid",
      meta: [ "€600.00 of €1,500.00 · €900.00 still outstanding" ],
      actions: [ :open_statement ]
    )
    render Campbooks::Money::NeedsYou.new(items: [ item ], overflow: 0, statement: stub_statement("January", "Millennium BCP"))
  end

  # Needs-you section — NIF kind (ask for corrected invoice).
  # @label NeedsYou (nif)
  def needs_you_nif
    txn = stub_txn(counterparty: "Staples Portugal", amount_cents: -36_400, booked_on: Date.new(2024, 1, 20))
    item = Money::NeedsYouItem.new(
      kind: :nif, transaction: txn, match: nil, group: nil,
      title: "Invoice needs a NIF",
      meta: [ "Staples Portugal", "€364.00" ],
      actions: [ :request_invoice ]
    )
    render Campbooks::Money::NeedsYou.new(items: [ item ], overflow: 2, statement: stub_statement("January", "Millennium BCP"))
  end

  # Unbanked list with two missing obligations.
  # @label Unbanked (with rows)
  def unbanked_with_rows
    obligations = [
      stub_obligation(:payable, "Galp Frota", "FT2024/0902", 8_860, Date.new(2024, 1, 22), "January"),
      stub_obligation(:receivable, "Acme Consulting", "0234", 222_000, Date.new(2024, 1, 14), "January")
    ]
    evidence = OpenStruct.new(any?: true)
    render Campbooks::Money::Unbanked.new(obligations: obligations, evidence: evidence)
  end

  # Unbanked empty state — all invoices accounted for.
  # @label Unbanked (empty)
  def unbanked_empty
    evidence = OpenStruct.new(any?: true)
    render Campbooks::Money::Unbanked.new(obligations: [], evidence: evidence)
  end

  private

  def stub_read(**attrs)
    OpenStruct.new(**attrs)
  end

  def stub_statement(label, bank)
    OpenStruct.new(period_label: label, bank_name: bank, period_start: Date.new(2024, 1, 1), period_end: Date.new(2024, 1, 31), id: 0)
  end

  def stub_txn(counterparty:, amount_cents:, booked_on:)
    OpenStruct.new(id: rand(1_000_000), counterparty: counterparty,
                   amount_cents: amount_cents, booked_on: booked_on,
                   debit?: amount_cents.negative?,
                   unmatched?: true, suggested?: false, matched?: false)
  end

  def stub_obligation(direction, counterpart, invoice_number, cents, anchor_on, label)
    Money::Obligation.new(
      id: "doc:#{rand(100_000)}", direction: direction, counterpart: counterpart,
      what: "Invoice ##{invoice_number}", amount: ::Money.new(cents, "EUR"),
      anchor_on: anchor_on, status: :missing, settled_on: nil, settled_via: nil,
      source_email_message: nil, document: nil, statement: nil,
      statement_label: label, actions: direction == :payable ? %i[paid_elsewhere mark_paid] : %i[send_reminder mark_paid]
    )
  end
end
