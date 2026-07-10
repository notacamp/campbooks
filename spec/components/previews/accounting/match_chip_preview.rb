# frozen_string_literal: true

module Accounting
  class MatchChipPreview < Lookbook::Preview
    # Confirmed match — green chip.
    def confirmed
      render(Campbooks::Accounting::MatchChip.new(match: stub_match(:confirmed)))
    end

    # Suggested match — amber chip.
    def suggested
      render(Campbooks::Accounting::MatchChip.new(match: stub_match(:suggested)))
    end

    # Confirmed match with NIF mismatch warning.
    def with_nif_mismatch
      render(Campbooks::Accounting::MatchChip.new(
               match:      stub_match(:confirmed),
               nif_status: :mismatch))
    end

    # Confirmed match with missing NIF warning.
    def with_nif_missing
      render(Campbooks::Accounting::MatchChip.new(
               match:      stub_match(:confirmed),
               nif_status: :missing))
    end

    private

    def stub_document
      Document.new(
        document_type: :expense_invoice,
        document_date: Date.new(2025, 6, 15),
        invoice_number: "INV-2025-042",
        ai_status: :complete
      ).tap do |d|
        # Simulate display_title
        d.define_singleton_method(:display_title) { "Acme Corp — INV-2025-042" }
      end
    end

    def stub_match(status_sym)
      TransactionMatch.new(
        status:        status_sym,
        matched_by:    :heuristic,
        confidence:    status_sym == :confirmed ? 0.91 : 0.76
      ).tap do |m|
        m.define_singleton_method(:document) { stub_document }
      end
    end
  end
end
