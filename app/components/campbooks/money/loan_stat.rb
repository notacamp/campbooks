# frozen_string_literal: true

module Campbooks
  module Money
    # The loan's stat in the Money strip: "Loan · 18 of 60 paid · €32,760 to go · next 5 Oct".
    # Reads only Loan's public helpers so the Lookbook preview can pass a stub.
    #
    # @param loan [Loan]
    class LoanStat < Campbooks::Base
      def initialize(loan:)
        @loan = loan
      end

      def view_template
        a(href: "#money_loan", class: "group flex flex-col no-underline transition-opacity hover:opacity-80") do
          div(class: "text-[11px] font-semibold uppercase tracking-widest text-muted-foreground") { t(".label") }
          div(class: "mt-1.5 text-[16px] font-semibold tabular-nums text-foreground") do
            plain t(".paid_of", paid: @loan.paid_count, total: @loan.term_months)
          end
          div(class: "mt-0.5 text-[12px] text-muted-foreground") do
            nxt   = @loan.next_expected
            parts = []
            parts << t(".to_go", amount: amount(@loan.remaining_cents)) if @loan.remaining_cents.positive?
            parts << t(".next", date: l(nxt.expected_on, format: :day_month_short)) if nxt
            plain parts.join(" · ")
          end
        end
      end

      private

      def amount(cents)
        ::Money.new(cents, @loan.currency.presence || "EUR").format(no_cents_if_whole: true)
      end
    end
  end
end
