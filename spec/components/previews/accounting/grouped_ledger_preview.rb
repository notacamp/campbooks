# frozen_string_literal: true

module Accounting
  # Preview for GroupedLedger — the full container with multiple group types.
  class GroupedLedgerPreview < Lookbook::Preview
    # Empty state — no groups.
    def empty
      render Campbooks::Accounting::GroupedLedger.new(
        groups:          [],
        reconciliation:  stub_reconciliation,
        company_nif:     nil
      )
    end

    # Full statement — representative mix of every group state.
    def full_statement
      render Campbooks::Accounting::GroupedLedger.new(
        groups:          all_groups,
        reconciliation:  stub_reconciliation,
        company_nif:     nil
      )
    end

    private

    def stub_reconciliation
      Reconciliation.new.tap { |r| r.id = "00000000-0000-0000-0000-000000000099" }
    end

    def all_groups
      [
        one_to_one_matched_group,
        many_to_one_group,
        one_to_many_group,
        partial_group,
        unmatched_group
      ]
    end

    def one_to_one_matched_group
      txn = stub_txn("Débito direto telecomunicações", "Vodafone Portugal",
                     -4590, Date.new(2024, 1, 5), :matched)
      doc = stub_doc("Vodafone Portugal", "FT2024/00045", 4590)
      wire_match(txn, doc, :confirmed, 4590)
      Reconciliations::Groups::Group.new(
        bank_transactions: [ txn ], documents: [ doc ],
        line_total_cents: 4590, invoice_total_cents: 4590,
        allocated_cents: 4590, outstanding_cents: 0,
        balanced: true, kind: :one_to_one
      )
    end

    def many_to_one_group
      t1 = stub_txn("Equipamento (1/2)", "Insight IT", -60_000, Date.new(2024, 1, 8), :matched)
      t2 = stub_txn("Equipamento (2/2)", "Insight IT", -60_000, Date.new(2024, 1, 22), :matched)
      doc = stub_doc("Insight IT, Lda.", "FT2024/00300", 120_000)
      wire_match(t1, doc, :confirmed, 60_000)
      wire_match(t2, doc, :confirmed, 60_000)
      Reconciliations::Groups::Group.new(
        bank_transactions: [ t1, t2 ], documents: [ doc ],
        line_total_cents: 120_000, invoice_total_cents: 120_000,
        allocated_cents: 120_000, outstanding_cents: 0,
        balanced: true, kind: :many_to_one
      )
    end

    def one_to_many_group
      txn  = stub_txn("Transferência a fornecedores", "Grupo Norte",
                      -26_000, Date.new(2024, 1, 16), :matched)
      doc1 = stub_doc("Papelaria Central", "FT2024/00181", 18_000)
      doc2 = stub_doc("CTT Expresso",      "FT2024/00199", 8_000)
      wire_match(txn, doc1, :confirmed, 18_000)
      wire_match(txn, doc2, :confirmed, 8_000)
      Reconciliations::Groups::Group.new(
        bank_transactions: [ txn ], documents: [ doc1, doc2 ],
        line_total_cents: 26_000, invoice_total_cents: 26_000,
        allocated_cents: 26_000, outstanding_cents: 0,
        balanced: true, kind: :one_to_many
      )
    end

    def partial_group
      txn = stub_txn("Sinal de obra", "Studio Reno",
                     -60_000, Date.new(2024, 1, 12), :matched)
      doc = stub_doc("Studio Reno", "FT2024/00120", 150_000)
      wire_match(txn, doc, :confirmed, 60_000)
      Reconciliations::Groups::Group.new(
        bank_transactions: [ txn ], documents: [ doc ],
        line_total_cents: 60_000, invoice_total_cents: 150_000,
        allocated_cents: 60_000, outstanding_cents: 90_000,
        balanced: false, kind: :partial
      )
    end

    def unmatched_group
      txn = stub_txn("Pagamento fornecedor", "Distribuidora Norte",
                     -20_000, Date.new(2024, 1, 28), :unmatched)
      txn.association(:transaction_matches).target = []
      Reconciliations::Groups::Group.new(
        bank_transactions: [ txn ], documents: [],
        line_total_cents: 20_000, invoice_total_cents: 0,
        allocated_cents: 0, outstanding_cents: 0,
        balanced: false, kind: :unmatched
      )
    end

    # ── Stubs ──────────────────────────────────────────────────────────────

    def stub_txn(description, counterparty, amount_cents, booked_on, status)
      BankTransaction.new.tap do |t|
        t.id               = SecureRandom.uuid
        t.description      = description
        t.counterparty     = counterparty
        t.amount_cents     = amount_cents
        t.booked_on        = booked_on
        t.currency         = "EUR"
        t.status           = status
        t.association(:transaction_matches).target = []
      end
    end

    def stub_doc(vendor_name, invoice_number, amount_cents)
      Document.new.tap do |d|
        d.id            = SecureRandom.uuid
        d.document_type = :expense_invoice
        d.metadata      = {
          "vendor_name"    => vendor_name,
          "invoice_number" => invoice_number,
          "amount_cents"   => amount_cents.to_s
        }
      end
    end

    def wire_match(txn, doc, status, allocated_cents)
      match = TransactionMatch.new.tap do |m|
        m.id              = SecureRandom.uuid
        m.bank_transaction = txn
        m.document        = doc
        m.status          = status
        m.confidence      = 0.92
        m.allocated_cents = allocated_cents
      end
      txn.association(:transaction_matches).target << match
    end
  end
end
