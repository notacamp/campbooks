# frozen_string_literal: true

module Campbooks
  module Accounting
    # Refactored header block for the reconciliation show page.
    # Renders status badge + bank/period/balance/integrity chips.
    #
    # @param reconciliation [Reconciliation]
    class ReconciliationHeader < Campbooks::Base
      def initialize(reconciliation:)
        @reconciliation = reconciliation
      end

      def view_template
        div(class: "flex flex-wrap items-center gap-2 mb-6") do
          render(Campbooks::Accounting::ReconciliationStatusBadge.new(status: @reconciliation.status))

          if @reconciliation.bank_name.present?
            render(Campbooks::Badge.new(variant: :neutral)) { @reconciliation.bank_name }
          end

          if @reconciliation.period_label
            render(Campbooks::Badge.new(variant: :neutral)) { @reconciliation.period_label }
          end

          if @reconciliation.currency.present?
            render(Campbooks::Badge.new(variant: :neutral)) { @reconciliation.currency }
          end

          if @reconciliation.integrity_warning?
            render(Campbooks::Badge.new(variant: :warning)) do
              @reconciliation.integrity_warning_message.presence ||
                t("reconciliations.show.integrity_warning")
            end
          end
        end
      end
    end
  end
end
