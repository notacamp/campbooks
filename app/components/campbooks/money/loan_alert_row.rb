# frozen_string_literal: true

module Campbooks
  module Money
    # A "Needs you" row for a loan issue: a missed instalment or an unacknowledged
    # instalment amount change after a rate reset.
    #
    # @param loan        [Loan]
    # @param instalment  [LoanInstalment]
    # @param kind        [:missed | :changed]
    class LoanAlertRow < Campbooks::Base
      def initialize(loan:, instalment:, kind:)
        @loan        = loan
        @instalment  = instalment
        @kind        = kind
      end

      def view_template
        div(class: "flex items-center gap-3 py-3 px-4 -mx-4 rounded-xl hover:bg-muted/40 transition-colors") do
          span(class: "w-2 h-2 rounded-full bg-ember flex-none", aria_hidden: "true")

          div(class: "min-w-0 flex-1") do
            div(class: "text-[13.5px] font-semibold") { plain title_text }
            div(class: "text-[12.5px] text-muted-foreground mt-0.5") { plain meta_text }
          end

          div(class: "flex items-center gap-2 flex-none") do
            case @kind
            when :missed
              missed_actions
            when :changed
              changed_actions
            end
          end
        end
      end

      private

      def title_text
        case @kind
        when :missed
          t(".missed_title", month: month_label(@instalment.expected_on))
        when :changed
          t(".changed_title")
        end
      end

      def meta_text
        case @kind
        when :missed
          rec = @instalment.loan.workspace.reconciliations
                            .where(status: :ready)
                            .order(period_end: :desc)
                            .first
          rec_label = rec ? t(".statement_label", period: rec.period_label.to_s) : ""
          t(".missed_meta",
            date:      I18n.l(@instalment.expected_on, format: :date),
            amount:    format_cents(@instalment.amount_cents, @loan.currency),
            statement: rec_label)
        when :changed
          prev = @instalment.previous_amount_cents
          curr = @instalment.amount_cents
          t(".changed_meta",
            from:  format_cents(prev, @loan.currency),
            to:    format_cents(curr, @loan.currency),
            month: month_label(@instalment.expected_on))
        end
      end

      def missed_actions
        link_to t(".find_it"),
                helpers.money_loan_path(@loan),
                class: "inline-flex items-center px-3 py-1.5 rounded-lg bg-foreground text-background text-[12.5px] font-medium hover:opacity-90 transition-opacity"

        if @instalment.loan.workspace.reconciliations.where(status: :ready).any?
          link_to t(".it_was_paid"),
                  helpers.money_loan_path(@loan),
                  class: "inline-flex items-center px-3 py-1.5 rounded-lg border border-border text-[12.5px] font-medium text-muted-foreground hover:bg-muted/60 transition-colors"
        end
      end

      def changed_actions
        helpers.button_to(
          t(".fine"),
          helpers.money_loan_path(@loan),
          method: :patch,
          params: { loan: { change_acknowledged_at: Time.current.iso8601 } },
          class: "inline-flex items-center px-3 py-1.5 rounded-lg bg-foreground text-background text-[12.5px] font-medium hover:opacity-90 transition-opacity",
          form: { data: { turbo: true } }
        )
      end

      def month_label(date)
        I18n.l(date, format: :month_year)
      end

      def format_cents(cents, currency)
        return "-" if cents.nil?

        symbol = { "EUR" => "€", "USD" => "$", "GBP" => "£" }.fetch(currency.to_s.upcase, currency)
        "#{symbol}#{sprintf("%.2f", cents.abs / 100.0)}"
      end
    end
  end
end
