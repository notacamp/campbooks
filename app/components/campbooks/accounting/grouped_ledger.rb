# frozen_string_literal: true

module Campbooks
  module Accounting
    # Container that renders all reconciliation groups as a paired ledger.
    #
    # Each group shows bank line(s) on the left, a state-glyph node in the
    # middle, and matched invoice(s) on the right. Simple 1:1 groups are flat
    # rows; multi-item groups (installments, split payments, partials) get an
    # inset container with a balance footer.
    #
    # @param groups         [Array<Reconciliations::Groups::Group>]
    # @param reconciliation [Reconciliation]   (for the company NIF)
    # @param company_nif    [String, nil]
    class GroupedLedger < Campbooks::Base
      def initialize(groups:, reconciliation:, company_nif: nil)
        @groups          = groups
        @reconciliation  = reconciliation
        @company_nif     = company_nif.presence
      end

      def view_template
        if @groups.empty?
          render Campbooks::EmptyState.new(
            variant: :card,
            title:   t(".empty")
          )
          return
        end

        div(class: "divide-y divide-border") do
          @groups.each do |group|
            render ReconciliationGroup.new(
              group:          group,
              company_nif:    @company_nif,
              reconciliation: @reconciliation
            )
          end
        end
      end
    end
  end
end
