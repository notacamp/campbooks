# frozen_string_literal: true

module Campbooks
  module Accounting
    # Renders a confidence indicator for a TransactionMatch.
    # Wording comes first, percentage second.
    # On desktop: confidence number shown as a title attribute.
    # On mobile: tappable block expands to show reasons.
    #
    # @param match        [TransactionMatch]
    # @param expandable   [Boolean] whether to include the mobile tap-to-expand
    class ConfidenceBadge < Campbooks::Base
      def initialize(match:, expandable: true)
        @match      = match
        @expandable = expandable
      end

      def view_template
        pct  = ((@match.confidence || 0) * 100).round
        wording = wording_for(@match.confidence)

        div(class: "group relative") do
          span(
            class: badge_classes,
            title: "#{wording}: #{pct}%"
          ) do
            plain "#{wording} · #{pct}%"
          end

          # Reasons block (always rendered, toggled via Stimulus or CSS)
          if @expandable && @match.match_reasons.present?
            div(class: "hidden group-focus-within:block md:hidden mt-1 text-xs text-muted-foreground space-y-0.5") do
              reason_lines.each { |line| p(class: "leading-snug") { line } }
            end
          end

          # Desktop: reasons as a tooltip on the title attr (no JS needed)
          nil
        end
      end

      private

      def wording_for(confidence)
        confidence = confidence.to_f
        if confidence >= 0.8
          t(".strong")
        elsif confidence >= 0.6
          t(".possible")
        else
          t(".weak")
        end
      end

      def badge_classes
        confidence = @match.confidence.to_f
        base = "inline-flex items-center text-xs font-medium px-1.5 py-0.5 rounded cursor-help"
        if confidence >= 0.8
          "#{base} tone-green"
        elsif confidence >= 0.6
          "#{base} tone-amber"
        else
          "#{base} tone-neutral"
        end
      end

      def reason_lines
        reasons = @match.match_reasons || {}
        lines   = []

        case reasons["amount"]
        when "exact"
          lines << t(".reason_amount_exact")
        when "close"
          lines << t(".reason_amount_close")
        end

        if (delta = reasons["date_delta_days"])
          lines << t(".reason_date", days: delta.to_i)
        end

        if (sim = reasons["name_similarity"])
          pct = (sim.to_f * 100).round
          lines << t(".reason_name", pct: pct)
        end

        if reasons["cross_reconciliation_warning"]
          lines << t(".reason_cross_reconciliation")
        end

        if (ai_reason = reasons["ai_reason"].presence)
          lines << ai_reason
        end

        lines
      end
    end
  end
end
