# frozen_string_literal: true

module Accounting
  # Preview all ReconciliationGroup states.
  # Groups are assembled from stubs so the preview has no DB dependency.
  class ReconciliationGroupPreview < Lookbook::Preview
    # 1:1 matched — a simple Vodafone debit against one invoice.
    def one_to_one_matched
      render Campbooks::Accounting::ReconciliationGroup.new(
        group: stub_group(:one_to_one,
                          txns: [ stub_txn(description: "Débito direto telecomunicações",
                                           counterparty: "Vodafone Portugal",
                                           amount_cents: -4590, booked_on: Date.new(2024, 1, 5),
                                           status: :matched) ],
                          docs: [ stub_doc(vendor_name: "Vodafone Portugal",
                                           invoice_number: "FT2024/00045",
                                           amount_cents: 4590) ],
                          match_status: :confirmed),
        company_nif: nil
      )
    end

    # Many → 1 matched — two installment card debits pay one Insight IT invoice.
    def many_to_one_matched
      render Campbooks::Accounting::ReconciliationGroup.new(
        group: stub_group(:many_to_one,
                          txns: [
                            stub_txn(description: "Equipamento (1/2)", counterparty: "Insight IT",
                                     amount_cents: -60_000, booked_on: Date.new(2024, 1, 8),
                                     status: :matched),
                            stub_txn(description: "Equipamento (2/2)", counterparty: "Insight IT",
                                     amount_cents: -60_000, booked_on: Date.new(2024, 1, 22),
                                     status: :matched)
                          ],
                          docs: [ stub_doc(vendor_name: "Insight IT, Lda.",
                                           invoice_number: "FT2024/00300",
                                           amount_cents: 120_000) ],
                          match_status: :confirmed),
        company_nif: nil
      )
    end

    # 1 → Many matched — one transfer clearing two invoices.
    def one_to_many_matched
      render Campbooks::Accounting::ReconciliationGroup.new(
        group: stub_group(:one_to_many,
                          txns: [ stub_txn(description: "Transferência a fornecedores",
                                           counterparty: "Grupo Norte",
                                           amount_cents: -26_000, booked_on: Date.new(2024, 1, 16),
                                           status: :matched) ],
                          docs: [
                            stub_doc(vendor_name: "Papelaria Central",
                                     invoice_number: "FT2024/00181", amount_cents: 18_000),
                            stub_doc(vendor_name: "CTT Expresso",
                                     invoice_number: "FT2024/00199", amount_cents: 8_000)
                          ],
                          match_status: :confirmed),
        company_nif: nil
      )
    end

    # Partial payment — deposit covers part of one invoice.
    def partial_payment
      render Campbooks::Accounting::ReconciliationGroup.new(
        group: stub_group(:partial,
                          txns: [ stub_txn(description: "Sinal de obra",
                                           counterparty: "Studio Reno",
                                           amount_cents: -60_000, booked_on: Date.new(2024, 1, 12),
                                           status: :matched) ],
                          docs: [ stub_doc(vendor_name: "Studio Reno",
                                           invoice_number: "FT2024/00120",
                                           amount_cents: 150_000) ],
                          match_status: :confirmed,
                          allocated_cents: 60_000),
        company_nif: nil
      )
    end

    # Review match with NIF flag — Staples suggested at 78% confidence.
    def review_with_nif
      render Campbooks::Accounting::ReconciliationGroup.new(
        group: stub_group(:one_to_one,
                          txns: [ stub_txn(description: "Material de escritório",
                                           counterparty: "Staples Portugal",
                                           amount_cents: -8432, booked_on: Date.new(2024, 1, 14),
                                           status: :suggested) ],
                          docs: [ stub_doc(vendor_name: "Staples Portugal",
                                           invoice_number: "FT2024/00221",
                                           amount_cents: 8432) ],
                          match_status: :suggested,
                          confidence: 0.78),
        company_nif: "PT500000001"  # triggers NIF flag (stub_doc has no NIF)
      )
    end

    # Credit / revenue — incoming transfer matched to a revenue invoice.
    def credit_matched
      render Campbooks::Accounting::ReconciliationGroup.new(
        group: stub_group(:one_to_one,
                          txns: [ stub_txn(description: "Transferência recebida",
                                           counterparty: "Acme Consulting",
                                           amount_cents: 150_000, booked_on: Date.new(2024, 1, 18),
                                           status: :matched, credit: true) ],
                          docs: [ stub_doc(vendor_name: "Acme Consulting",
                                           invoice_number: "FR2024/00008",
                                           amount_cents: 150_000,
                                           document_type: :revenue_invoice) ],
                          match_status: :confirmed),
        company_nif: nil
      )
    end

    # Excluded / set aside — a bank fee with no invoice needed.
    def excluded
      render Campbooks::Accounting::ReconciliationGroup.new(
        group: stub_group(:unmatched,
                          txns: [ stub_txn(description: "Comissão manutenção conta",
                                           counterparty: "Millennium BCP",
                                           amount_cents: -350, booked_on: Date.new(2024, 1, 25),
                                           status: :excluded,
                                           exclusion_reason: "bank_fee") ],
                          docs: []),
        company_nif: nil
      )
    end

    # Unmatched — no invoice found, needs resolution.
    def unmatched_needs_you
      render Campbooks::Accounting::ReconciliationGroup.new(
        group: stub_group(:unmatched,
                          txns: [ stub_txn(description: "Pagamento fornecedor",
                                           counterparty: "Distribuidora Norte",
                                           amount_cents: -20_000, booked_on: Date.new(2024, 1, 28),
                                           status: :unmatched) ],
                          docs: []),
        company_nif: nil
      )
    end

    private

    # ── Stub helpers ─────────────────────────────────────────────────────────

    def stub_txn(description:, counterparty:, amount_cents:, booked_on:,
                 status: :matched, credit: nil, exclusion_reason: nil)
      BankTransaction.new.tap do |t|
        t.id                = SecureRandom.uuid
        t.description       = description
        t.counterparty      = counterparty
        t.amount_cents      = amount_cents
        t.booked_on         = booked_on
        t.currency          = "EUR"
        t.status            = status
        t.exclusion_reason  = exclusion_reason
        # transaction_matches will be set by stub_group below
        t.association(:transaction_matches).target = []
      end
    end

    def stub_doc(vendor_name:, invoice_number:, amount_cents:,
                 document_type: :expense_invoice)
      Document.new.tap do |d|
        d.id              = SecureRandom.uuid
        d.document_type   = document_type
        d.metadata        = {
          "vendor_name"      => vendor_name,
          "invoice_number"   => invoice_number,
          "amount_cents"     => amount_cents.to_s
        }
      end
    end

    def stub_match(txn:, doc:, status:, confidence: 0.9, allocated_cents: nil)
      TransactionMatch.new.tap do |m|
        m.id              = SecureRandom.uuid
        m.bank_transaction = txn
        m.document        = doc
        m.status          = status
        m.confidence      = confidence
        m.allocated_cents = allocated_cents || doc.amount_cents
      end
    end

    def stub_group(kind, txns:, docs:, match_status: :confirmed,
                   confidence: 0.9, allocated_cents: nil)
      # Wire transaction_matches onto each txn
      txns.each do |txn|
        matches = docs.map { |doc|
          stub_match(txn: txn, doc: doc, status: match_status,
                     confidence: confidence,
                     allocated_cents: allocated_cents || doc.amount_cents)
        }
        txn.association(:transaction_matches).target = matches
      end

      line_total    = txns.sum { |t| t.amount_cents.abs }
      invoice_total = docs.sum { |d| d.amount_cents.to_i }
      alloc         = allocated_cents&.*(docs.size) || invoice_total
      outstanding   = [ invoice_total - aloc_for(kind, alloc, invoice_total), 0 ].max

      Reconciliations::Groups::Group.new(
        bank_transactions:   txns,
        documents:           docs,
        line_total_cents:    line_total,
        invoice_total_cents: invoice_total,
        allocated_cents:     alloc,
        outstanding_cents:   outstanding,
        balanced:            (line_total - invoice_total).abs <= 1,
        kind:                kind
      )
    end

    def aloc_for(kind, alloc, invoice_total)
      kind == :partial ? alloc : invoice_total
    end
  end
end
