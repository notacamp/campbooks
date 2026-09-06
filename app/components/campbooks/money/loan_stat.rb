# frozen_string_literal: true

module Campbooks
  module Money
    # Compact strip statistic for a single tracked loan.
    # Shows "Loan · N of T paid · €X to go · next D Mon".
    #
    # Used as a standalone before PR A's Strip/Read are merged.
    # @param loan [Loan]
    class LoanStat < Campbooks::Base
      def initialize(loan:)
        @loan = loan
      end

      def view_template
        div(class: "flex flex-col gap-0.5") do
          span(class: "text-[11px] font-bold tracking-widest uppercase text-muted-foreground") { plain t(".label") }
          span(class: "text-[15px] font-semibold tabular-nums") do
            plain t(".paid_of", paid: @loan.paid_count, total: @loan.term_months)
          end
          span(class: "text-[12px] text-muted-foreground mt-0.5") do
            parts = []
            parts << t(".to_go", amount: format_cents(@loan.remaining_cents, @loan.currency)) if @loan.remaining_cents > 0
            if (nxt = @loan.next_expected)
              parts << t(".next", date: I18n.l(nxt.expected_on, format: :day_month_short))
            end
            plain parts.join(" · ")
          end
        end
      end

      private

      def format_cents(cents, currency)
        symbol = { "EUR" => "€", "USD" => "$", "GBP" => "£" }.fetch(currency.to_s.upcase, currency)
        whole = cents % 100 == 0
        whole ? "#{symbol}#{cents / 100}" : "#{symbol}#{sprintf("%.2f", cents / 100.0)}"
      end
    end
  end
end
