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

  # Scout read when the month to reconcile has no statement yet.
  # @label Read (month to reconcile open)
  def read_focus_missing
    read = stub_read(
      any_statements?: true,
      focus_label:     "August",
      focus_reconciled?: false,
      statement:       stub_statement("July", "Millennium BCP"),
      statement_label: "July",
      bank_name:       "Millennium BCP",
      lines_total:     18,
      lines_explained: 18,
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

  # Scout read when Scout is holding emailed statements nobody reconciled.
  # @label Read (statements held)
  def read_pending_statements
    read = stub_read(
      any_statements?: true,
      focus_label:     "August",
      focus_reconciled?: false,
      pending_statement_count: 3,
      statement:       stub_statement("November 2025", "Millennium BCP"),
      statement_label: "November 2025",
      bank_name:       "Millennium BCP",
      lines_total:     9,
      lines_explained: 0,
      review_count:    0,
      needs_invoice_count: 6,
      needs_invoice_cents: 148_600,
      partial_count:   0,
      nif_count:       0,
      missing_count:   4,
      missing_in_newest_count: 4,
      missing_cents:   62_000,
      missing_cents_by_currency: { "EUR" => 62_000 },
      primary_currency: "EUR",
      explained_pct:   0,
      requested_count: 0
    )
    render Campbooks::Money::Read.new(read: read, class: "max-w-[700px]")
  end

  # Strip with the month-to-reconcile stat leading.
  # @label Strip (month to reconcile open)
  def strip_focus_missing
    read = stub_read(
      any_statements?: true,
      focus_label:     "August",
      focus_reconciled?: false,
      statement:       stub_statement("July", "Millennium BCP"),
      statement_label: "July",
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

  # Needs-you section — statements Scout holds (reconcile them / pick which).
  # @label NeedsYou (statements held)
  def needs_you_reconcile_statements
    docs = [
      stub_statement_document("Extrato Millennium BCP · Agosto 2026", Date.new(2026, 9, 2)),
      stub_statement_document("Extrato Millennium BCP · Julho 2026", Date.new(2026, 8, 3)),
      stub_statement_document("Extrato Millennium BCP · Junho 2026", Date.new(2026, 7, 2)),
      stub_statement_document("Extrato Millennium BCP · Maio 2026", Date.new(2026, 6, 2))
    ]
    item = Money::NeedsYouItem.new(kind: :reconcile_statements, payload: { documents: docs, count: 4 })
    render Campbooks::Money::NeedsYou.new(items: [ item ], overflow: 0, statement: stub_statement("November 2025", "Millennium BCP"))
  end

  # Needs-you section — a statement that couldn't be read (Try again).
  # @label NeedsYou (statement failed)
  def needs_you_statement_failed
    recon = OpenStruct.new(id: 7, period_label: nil, created_at: Time.new(2026, 9, 6, 18, 17),
                           parse_error: "Scout couldn't reach the AI provider after several tries (it was rate limiting us). The statement is fine. Try again in a few minutes.",
                           statement_document: OpenStruct.new(display_title: "Extrato Millennium BCP · Agosto 2026"),
                           model_name: Reconciliation.model_name, to_key: [ 7 ], to_param: "7")
    item = Money::NeedsYouItem.new(kind: :statement_failed, payload: { reconciliation: recon })
    render Campbooks::Money::NeedsYou.new(items: [ item ], overflow: 0, statement: stub_statement("July", "Millennium BCP"))
  end

  # Needs-you section — the month to reconcile has no statement anywhere.
  # @label NeedsYou (add a statement)
  def needs_you_add_statement
    item = Money::NeedsYouItem.new(kind: :add_statement, payload: { month: Date.new(2026, 8, 1), label: "August" })
    render Campbooks::Money::NeedsYou.new(items: [ item ], overflow: 0, statement: stub_statement("July", "Millennium BCP"))
  end

  # Month pills: the month to reconcile first, gaps folded, newest statement selected.
  # @label StatementTabs
  def statement_tabs
    render Campbooks::Money::StatementTabs.new(page: stub_months_page)
  end

  # Month pills with an older statement selected (after a click).
  # @label StatementTabs (older selected)
  def statement_tabs_older_selected
    render Campbooks::Money::StatementTabs.new(page: stub_months_page(selected: :jun))
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
    defaults = { focus_label: "January", focus_reconciled?: true, pending_statement_count: 0,
                 missing_in_newest_count: attrs[:missing_count].to_i, loans: [], loan_state: :none }
    OpenStruct.new(**defaults.merge(attrs))
  end

  # Stubs carry model_name/to_key/to_param so dom_id and the route helpers work.
  def stub_statement(label, bank, id: 0, period_start: Date.new(2024, 1, 1), period_end: Date.new(2024, 1, 31))
    OpenStruct.new(period_label: label, bank_name: bank, period_start: period_start, period_end: period_end, id: id,
                   model_name: Reconciliation.model_name, to_key: [ id ], to_param: id.to_s)
  end

  def stub_statement_document(title, filed_on)
    OpenStruct.new(display_title: title, created_at: filed_on.to_time)
  end

  # Evidence stand-in for the pills: today is 6 Sep 2026, statements for June,
  # July (twice) and November 2025, nothing since.
  StubEvidence = Struct.new(:today, :month_to_reconcile) do
    def month_label(date)
      name = I18n.l(date, format: :month_name)
      date.year == today.year ? name : "#{name} #{date.year}"
    end

    def label_for(statement) = month_label(statement.period_start)
  end

  def stub_months_page(selected: :nov)
    today = Date.new(2026, 9, 6)
    jun   = stub_statement("June 2025", "Millennium BCP", id: 1, period_start: Date.new(2025, 6, 1), period_end: Date.new(2025, 6, 30))
    jul_a = stub_statement("July 2025", "Millennium BCP", id: 2, period_start: Date.new(2025, 7, 1), period_end: Date.new(2025, 7, 31))
    jul_b = stub_statement("July 2025", "Millennium BCP", id: 3, period_start: Date.new(2025, 7, 1), period_end: Date.new(2025, 7, 31))
    nov   = stub_statement("November 2025", "Millennium BCP", id: 4, period_start: Date.new(2025, 11, 1), period_end: Date.new(2025, 11, 30))
    by_month = { Date.new(2025, 6, 1) => [ jun ], Date.new(2025, 7, 1) => [ jul_a, jul_b ], Date.new(2025, 11, 1) => [ nov ] }

    months = []
    cursor = Date.new(2026, 8, 1)
    while cursor >= Date.new(2025, 6, 1)
      months << Money::Evidence::Month.new(starts_on: cursor, statements: by_month.fetch(cursor, []))
      cursor = cursor.prev_month
    end

    OpenStruct.new(
      months:             months,
      evidence:           StubEvidence.new(today, Date.new(2026, 8, 1)),
      selected_statement: selected == :jun ? jun : nov,
      statement_counts:   { 1 => [ 3, 3 ], 2 => [ 4, 4 ], 3 => [ 2, 4 ], 4 => [ 0, 9 ] }
    )
  end

  def stub_txn(counterparty:, amount_cents:, booked_on:)
    id = rand(1_000_000)
    OpenStruct.new(id: id, counterparty: counterparty,
                   amount_cents: amount_cents, booked_on: booked_on,
                   debit?: amount_cents.negative?,
                   unmatched?: true, suggested?: false, matched?: false,
                   model_name: BankTransaction.model_name, to_key: [ id ], to_param: id.to_s)
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
