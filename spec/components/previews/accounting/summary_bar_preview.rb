# frozen_string_literal: true

module Accounting
  class SummaryBarPreview < Lookbook::Preview
    # All zeros — initial state before matching runs.
    def empty
      render(Campbooks::Accounting::SummaryBar.new(
               reconciliation:      stub_reconciliation,
               status_counts:       {},
               nif_exception_count: 0))
    end

    # Typical mid-reconciliation state: some matched, some suggested, many unmatched.
    def in_progress
      render(Campbooks::Accounting::SummaryBar.new(
               reconciliation:      stub_reconciliation,
               status_counts:       { "matched" => 12, "suggested" => 7, "unmatched" => 23, "excluded" => 2 },
               nif_exception_count: 0))
    end

    # With NIF exceptions highlighted.
    def with_nif_exceptions
      render(Campbooks::Accounting::SummaryBar.new(
               reconciliation:      stub_reconciliation,
               status_counts:       { "matched" => 18, "suggested" => 5, "unmatched" => 6, "excluded" => 1 },
               nif_exception_count: 3))
    end

    # Fully resolved — no unmatched or suggested, no confirm-all button.
    def fully_resolved
      render(Campbooks::Accounting::SummaryBar.new(
               reconciliation:      stub_reconciliation,
               status_counts:       { "matched" => 28, "excluded" => 6 },
               nif_exception_count: 0))
    end

    private

    def stub_reconciliation
      Reconciliation.new.tap do |r|
        r.id = "00000000-0000-0000-0000-000000000001"
      end
    end
  end
end
