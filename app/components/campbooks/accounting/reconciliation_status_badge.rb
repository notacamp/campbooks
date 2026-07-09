# frozen_string_literal: true

module Campbooks
  module Accounting
    # Chip badge that colour-codes a Reconciliation's status.
    #
    #   render(Campbooks::Accounting::ReconciliationStatusBadge.new(status: "ready"))
    #
    # @param status [String, Symbol] one of Reconciliation.statuses.keys
    class ReconciliationStatusBadge < Campbooks::Base
      # Maps status → Badge :variant
      VARIANT = {
        "pending"  => :neutral,
        "parsing"  => :info,
        "matching" => :info,
        "ready"    => :success,
        "failed"   => :danger
      }.freeze

      def initialize(status:)
        @status = status.to_s
      end

      def view_template
        variant = VARIANT.fetch(@status, :neutral)
        render(Campbooks::Badge.new(variant: variant)) { label }
      end

      private

      def label
        t(".#{@status}")
      end
    end
  end
end
