# frozen_string_literal: true

module Campbooks
  module Accounting
    # Status chip bar displayed above the transaction list.
    # Shows matched/suggested/unmatched/excluded counts plus an optional NIF
    # exception chip. When there are suggested matches, renders a "Confirm all N"
    # ghost button.
    #
    # id="reconciliation_summary_bar" is the Turbo broadcast target.
    #
    # @param reconciliation      [Reconciliation]
    # @param status_counts       [Hash] { "matched" => N, "suggested" => N, ... }
    #                            from BankTransaction.group(:status).count
    # @param nif_exception_count [Integer] matched/suggested txns whose top document
    #                            has nif_status :missing or :mismatch
    class SummaryBar < Campbooks::Base
      def initialize(reconciliation:, status_counts: {}, nif_exception_count: 0)
        @reconciliation      = reconciliation
        @status_counts       = status_counts.transform_keys(&:to_s)
        @nif_exception_count = nif_exception_count.to_i
      end

      def view_template
        div(id: "reconciliation_summary_bar",
            class: "flex flex-wrap items-center gap-2 mb-4") do
          matched_chip
          suggested_chip
          unmatched_chip
          excluded_chip
          requested_chip
          nif_chip
          confirm_all_button
        end
      end

      private

      def count(status)
        @status_counts[status.to_s].to_i
      end

      def suggested_count
        count(:suggested)
      end

      def matched_chip
        n = count(:matched)
        return if n.zero?

        render(Campbooks::Badge.new(variant: :success)) do
          "#{n} #{t(".matched", count: n)}"
        end
      end

      def suggested_chip
        n = suggested_count
        return if n.zero?

        render(Campbooks::Badge.new(variant: :warning)) do
          "#{n} #{t(".to_review", count: n)}"
        end
      end

      def unmatched_chip
        n = count(:unmatched)
        return if n.zero?

        render(Campbooks::Badge.new(variant: :neutral)) do
          "#{n} #{t(".unmatched", count: n)}"
        end
      end

      def excluded_chip
        n = count(:excluded)
        return if n.zero?

        render(Campbooks::Badge.new(variant: :neutral)) do
          "#{n} #{t(".excluded", count: n)}"
        end
      end

      def requested_chip
        n = count(:requested)
        return if n.zero?

        render(Campbooks::Badge.new(variant: :info)) do
          "#{n} #{t(".requested", count: n)}"
        end
      end

      def nif_chip
        return if @nif_exception_count.zero?

        span(class: "inline-flex items-center gap-1 px-2 py-0.5 text-xs font-medium rounded-md tone-amber",
             title: t(".nif_exceptions_title")) do
          # Warning icon
          svg(class: "w-3.5 h-3.5", viewBox: "0 0 20 20", fill: "currentColor") do |s|
            s.path(fill_rule: "evenodd",
                   d: "M8.485 2.495c.673-1.167 2.357-1.167 3.03 0l6.28 10.875c.673 1.167-.17 2.625-1.516 2.625H3.72c-1.347 0-2.189-1.458-1.515-2.625L8.485 2.495zM10 5a.75.75 0 01.75.75v3.5a.75.75 0 01-1.5 0v-3.5A.75.75 0 0110 5zm0 9a1 1 0 100-2 1 1 0 000 2z",
                   clip_rule: "evenodd")
          end
          "#{@nif_exception_count} #{t(".nif_exceptions", count: @nif_exception_count)}"
        end
      end

      def confirm_all_button
        return if suggested_count.zero?

        confirm_msg = t(".confirm_all_confirm", count: suggested_count)

        render(Campbooks::Button.new(
          variant: :ghost,
          size:    :sm,
          href:    helpers.confirm_all_suggestions_reconciliation_path(@reconciliation),
          data:    {
            turbo_method:  :post,
            turbo_confirm: confirm_msg
          }
        )) { t(".confirm_all", count: suggested_count) }
      end
    end
  end
end
