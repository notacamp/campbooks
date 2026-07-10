# frozen_string_literal: true

module Accounting
  class ReconciliationStatusBadgePreview < Lookbook::Preview
    # Pending — just created, awaiting parse trigger.
    def pending
      render(Campbooks::Accounting::ReconciliationStatusBadge.new(status: :pending))
    end

    # Parsing — CSV/PDF is being read.
    def parsing
      render(Campbooks::Accounting::ReconciliationStatusBadge.new(status: :parsing))
    end

    # Matching — matching engine running (PR 2).
    def matching
      render(Campbooks::Accounting::ReconciliationStatusBadge.new(status: :matching))
    end

    # Ready — transactions loaded and ready for workbench.
    def ready
      render(Campbooks::Accounting::ReconciliationStatusBadge.new(status: :ready))
    end

    # Failed — parse or match error.
    def failed
      render(Campbooks::Accounting::ReconciliationStatusBadge.new(status: :failed))
    end
  end
end
