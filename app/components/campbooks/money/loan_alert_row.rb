# frozen_string_literal: true

module Campbooks
  module Money
    # A Needs-you row for something odd about the loan: an instalment a reconciled
    # statement should have shown but didn't (:missed), or an amount change after a
    # rate reset nobody has waved through yet (:changed).
    #
    # @param loan        [Loan]
    # @param instalment  [LoanInstalment]
    # @param kind        [:missed | :changed]
    class LoanAlertRow < Campbooks::Base
      include Campbooks::Money::WorkRowShell

      def initialize(loan:, instalment:, kind:)
        @loan       = loan
        @instalment = instalment
        @kind       = kind
      end

      def view_template
        div(class: WRAPPER_CLASSES) do
          work_row(title: title_text, meta: meta_parts) do
            @kind == :missed ? missed_actions : changed_actions
          end
        end
      end

      private

      def title_text
        if @kind == :missed
          t(".missed_title", month: l(@instalment.expected_on, format: :month_year))
        else
          t(".changed_title")
        end
      end

      def meta_parts
        if @kind == :missed
          parts = [ t(".expected", date: l(@instalment.expected_on, format: :date)), amount(@instalment.amount_cents) ]
          parts << t(".statement_label", period: covering_statement.period_label.to_s) if covering_statement
          parts
        else
          [ t(".changed_meta", from: amount(@instalment.previous_amount_cents), to: amount(@instalment.amount_cents),
                                month: l(@instalment.expected_on, format: :month_year)) ]
        end
      end

      def missed_actions
        render Campbooks::Button.new(variant: :outline, size: :sm, href: helpers.money_loan_path(@loan),
                                     data: { turbo_frame: "_top" }) { t(".it_was_paid") }
        target = covering_statement ? helpers.reconciliation_path(covering_statement) : helpers.money_loan_path(@loan)
        render Campbooks::Button.new(variant: :primary, size: :sm, href: target, data: { turbo_frame: "_top" }) { t(".find_it") }
      end

      def changed_actions
        post_form(helpers.money_loan_path(@loan), method: :patch,
                  hidden: { "loan[change_acknowledged_at]" => Time.current.iso8601 }) do
          render(Campbooks::Button.new(variant: :primary, size: :sm, type: "submit")) { t(".fine") }
        end
      end

      # The reconciled statement whose period holds the instalment's date.
      def covering_statement
        return @covering_statement if defined?(@covering_statement)

        @covering_statement = @loan.workspace.reconciliations.where(status: :ready)
                                   .where("period_start <= ? AND period_end >= ?", @instalment.expected_on, @instalment.expected_on)
                                   .order(period_end: :desc).first
      end

      def amount(cents)
        return "-" if cents.nil?

        ::Money.new(cents, @loan.currency.presence || "EUR").format
      end
    end
  end
end
