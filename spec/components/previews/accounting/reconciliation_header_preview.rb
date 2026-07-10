# frozen_string_literal: true

module Accounting
  class ReconciliationHeaderPreview < Lookbook::Preview
    # Ready reconciliation with bank name, period, and currency.
    def ready
      render(Campbooks::Accounting::ReconciliationHeader.new(reconciliation: stub_reconciliation(:ready)))
    end

    # Parsing state.
    def parsing
      render(Campbooks::Accounting::ReconciliationHeader.new(reconciliation: stub_reconciliation(:parsing)))
    end

    # Matching state (PR 2).
    def matching
      render(Campbooks::Accounting::ReconciliationHeader.new(reconciliation: stub_reconciliation(:matching)))
    end

    # Failed state.
    def failed
      render(Campbooks::Accounting::ReconciliationHeader.new(reconciliation: stub_reconciliation(:failed)))
    end

    # Ready with integrity warning (balance mismatch).
    def with_integrity_warning
      r = stub_reconciliation(:ready)
      r.define_singleton_method(:integrity_warning?) { true }
      r.define_singleton_method(:integrity_warning_message) { "Closing balance mismatch: expected €1,240.00 but got €1,180.00" }
      render(Campbooks::Accounting::ReconciliationHeader.new(reconciliation: r))
    end

    private

    def stub_reconciliation(status_sym)
      Reconciliation.new(
        status:    status_sym,
        bank_name: "Millennium BCP",
        currency:  "EUR"
      ).tap do |r|
        r.define_singleton_method(:period_label) { "Jan – Jun 2025" }
        r.define_singleton_method(:integrity_warning?) { false }
        r.define_singleton_method(:integrity_warning_message) { nil }
      end
    end
  end
end
