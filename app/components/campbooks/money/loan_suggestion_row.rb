# frozen_string_literal: true

module Campbooks
  module Money
    # Scout's untracked-loan guess as a Needs-you row: "This looks like a loan
    # instalment", with "Not a loan" (dismiss) and "Track the loan", which discloses
    # the prefilled form under the row (a checkbox-peer disclosure, no JS).
    #
    # @param suggestion [Loans::Spotter::Suggestion]
    class LoanSuggestionRow < Campbooks::Base
      include Campbooks::Money::WorkRowShell
      include Campbooks::Money::Ordinals

      def initialize(suggestion:)
        @suggestion = suggestion
      end

      def view_template
        toggle_id = "loan_suggestion_#{@suggestion.key.to_s.parameterize}"

        div(class: WRAPPER_CLASSES) do
          input(type: "checkbox", id: toggle_id, class: "peer hidden")
          work_row(title: t(".title"), meta: meta_parts) do
            post_form(helpers.dismiss_money_loan_path, hidden: { key: @suggestion.key }) do
              render(Campbooks::Button.new(variant: :outline, size: :sm, type: "submit")) { t(".not_a_loan") }
            end
            label(for: toggle_id, class: class_names(button_classes(:primary), "peer-checked:hidden")) { t(".track") }
          end
          div(class: "hidden px-4 pb-4 peer-checked:block sm:px-6") do
            render Campbooks::Money::LoanForm.new(suggestion: @suggestion, toggle_id: toggle_id)
          end
        end
      end

      private

      def meta_parts
        amount = ::Money.new(@suggestion.instalment_cents, @suggestion.currency.presence || "EUR").format
        parts  = [ @suggestion.lender_guess, "−#{amount}" ]
        if @suggestion.first_seen_on && @suggestion.day_of_month
          parts << t(".every_month", day: day_label(@suggestion.day_of_month), count: @suggestion.count,
                                     since: l(@suggestion.first_seen_on, format: :month_year))
        end
        parts << t(".never_invoice")
        parts
      end
    end
  end
end
