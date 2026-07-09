# frozen_string_literal: true

module Accounting
  class ResolvePanelPreview < Lookbook::Preview
    STUB_RECON_ID = "00000000-1111-2222-3333-444444444444"
    STUB_TXN_ID   = "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee"

    # No suggestions yet — shows search + exclude sections only.
    def no_suggestions
      render(Campbooks::Accounting::ResolvePanel.new(
               transaction:         stub_debit_txn(matches_count: 0),
               reconciliation:      stub_recon,
               suggested_matches:   [],
               candidate_documents: [],
               company_nif:         nil))
    end

    # Scout found one high-confidence suggestion.
    def with_one_suggestion
      match = stub_match(0.91, :heuristic)
      render(Campbooks::Accounting::ResolvePanel.new(
               transaction:         stub_debit_txn(matches_count: 1),
               reconciliation:      stub_recon,
               suggested_matches:   [ match ],
               candidate_documents: [],
               company_nif:         nil))
    end

    # Scout found two suggestions; also shows document candidates in search.
    def with_suggestions_and_candidates
      matches = [
        stub_match(0.88, :heuristic),
        stub_match(0.62, :ai)
      ]
      render(Campbooks::Accounting::ResolvePanel.new(
               transaction:         stub_debit_txn(matches_count: 2),
               reconciliation:      stub_recon,
               suggested_matches:   matches,
               candidate_documents: stub_documents,
               company_nif:         nil))
    end

    # Suggestion with a NIF mismatch warning.
    def with_nif_mismatch_suggestion
      match = stub_match(0.85, :heuristic, buyer_nif: "PT999999990")
      render(Campbooks::Accounting::ResolvePanel.new(
               transaction:         stub_debit_txn(matches_count: 1),
               reconciliation:      stub_recon,
               suggested_matches:   [ match ],
               candidate_documents: [],
               company_nif:         "123456789"))
    end

    private

    def stub_recon
      Reconciliation.new.tap do |r|
        r.id = STUB_RECON_ID
      end
    end

    def stub_debit_txn(matches_count:)
      matches_proxy = Struct.new(:size) { }.new(matches_count)

      BankTransaction.new(
        description:  "Payment ACME LDA Invoice 2024-042",
        booked_on:    Date.new(2024, 6, 15),
        counterparty: "ACME LDA",
        amount_cents: -12_350,
        currency:     "EUR",
        status:       :unmatched
      ).tap do |t|
        t.id = STUB_TXN_ID
        t.define_singleton_method(:transaction_matches) { matches_proxy }
      end
    end

    def stub_match(confidence, matched_by, buyer_nif: nil)
      doc = stub_document(buyer_nif: buyer_nif)
      TransactionMatch.new(
        status:        :suggested,
        matched_by:    matched_by,
        confidence:    confidence,
        match_reasons: { "amount" => "exact", "date_delta_days" => 2 }
      ).tap do |m|
        m.define_singleton_method(:document) { doc }
      end
    end

    def stub_document(buyer_nif: nil)
      Document.new(
        document_type:  :expense_invoice,
        document_date:  Date.new(2024, 6, 12),
        invoice_number: "INV-2024-042",
        amount_cents:   12_350,
        currency:       "EUR",
        ai_status:      :complete
      ).tap do |d|
        d.buyer_nif = buyer_nif if buyer_nif
        d.define_singleton_method(:display_title) { "Acme LDA — INV-2024-042" }
        d.define_singleton_method(:classification) { nil }
      end
    end

    def stub_documents
      [
        Document.new(
          document_type:  :expense_invoice,
          document_date:  Date.new(2024, 5, 30),
          invoice_number: "INV-2024-039",
          amount_cents:   12_350,
          currency:       "EUR",
          ai_status:      :complete
        ).tap do |d|
          d.define_singleton_method(:display_title) { "Acme LDA — INV-2024-039" }
          d.define_singleton_method(:classification) { nil }
        end,
        Document.new(
          document_type:  :expense_invoice,
          document_date:  Date.new(2024, 6, 1),
          invoice_number: "SUP-8801",
          amount_cents:   8_000,
          currency:       "EUR",
          ai_status:      :complete
        ).tap do |d|
          d.define_singleton_method(:display_title) { "Beta Services — SUP-8801" }
          d.define_singleton_method(:classification) { nil }
        end
      ]
    end
  end
end
