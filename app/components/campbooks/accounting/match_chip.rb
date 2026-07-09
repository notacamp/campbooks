# frozen_string_literal: true

module Campbooks
  module Accounting
    # A chip displaying a matched/suggested document pairing.
    #
    # @param match       [TransactionMatch] the match record
    # @param nif_status  [Symbol, nil]      :ok, :missing, :mismatch, or nil
    class MatchChip < Campbooks::Base
      def initialize(match:, nif_status: nil)
        @match      = match
        @document   = match.document
        @nif_status = nif_status
      end

      def view_template
        div(class: chip_classes) do
          # Document type dot
          span(class: "w-1.5 h-1.5 rounded-full shrink-0 #{dot_color}", aria_hidden: "true")
          span(class: "min-w-0") do
            p(class: "text-xs font-medium truncate") { @document.display_title }
            p(class: "text-[10px] text-muted-foreground") { meta_line }
          end
          nif_indicator if @nif_status.in?(%i[missing mismatch])
        end
      end

      private

      def chip_classes
        base = "inline-flex items-center gap-1.5 px-2 py-1 rounded-md text-xs max-w-[220px]"
        case @match.status.to_sym
        when :confirmed then "#{base} tone-green"
        when :suggested then "#{base} tone-amber"
        else "#{base} tone-neutral"
        end
      end

      def dot_color
        case @match.status.to_sym
        when :confirmed then "bg-green-500"
        when :suggested then "bg-amber-500"
        else "bg-gray-400"
        end
      end

      def meta_line
        parts = []
        parts << @document.invoice_number if @document.invoice_number.present?
        parts << l(@document.document_date, format: :date) if @document.document_date.present?
        parts.join(" · ")
      end

      def nif_indicator
        variant = @nif_status == :mismatch ? "text-red-600 dark:text-red-400" : "text-amber-600 dark:text-amber-400"
        label   = @nif_status == :mismatch ? t(".nif_mismatch") : t(".nif_missing")

        span(class: "shrink-0 text-[10px] font-semibold #{variant}", title: label) { "NIF" }
      end
    end
  end
end
