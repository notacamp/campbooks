# frozen_string_literal: true

module Campbooks
  module Money
    # A "Needs you" work row that surfaces a Scout-spotted potential loan.
    # Shows a "Not a loan" dismiss button and a "Track the loan" button that
    # discloses a prefilled LoanForm.
    #
    # @param suggestion [Loans::Spotter::Suggestion]
    class LoanSuggestionRow < Campbooks::Base
      def initialize(suggestion:)
        @suggestion = suggestion
      end

      def view_template
        # Wrapper details element for the disclosure pattern
        div(class: "space-y-1") do
          # Work row
          div(class: "flex items-center gap-3 py-3 px-4 -mx-4 rounded-xl hover:bg-muted/40 transition-colors") do
            # Ember dot
            span(class: "w-2 h-2 rounded-full bg-ember flex-none",
                 style: "background: var(--ember-gradient, oklch(63% 0.21 20))",
                 aria_hidden: "true")

            div(class: "min-w-0 flex-1") do
              div(class: "text-[13.5px] font-semibold") { plain t(".title") }
              div(class: "text-[12.5px] text-muted-foreground mt-0.5 flex flex-wrap gap-x-2 gap-y-0.5") do
                plain @suggestion.lender_guess
                plain " · "
                plain helpers.number_to_currency(@suggestion.instalment_cents / 100.0, unit: "#{@suggestion.currency == 'EUR' ? '€' : @suggestion.currency}", precision: 2)
                if @suggestion.instalment_cents.positive?
                  plain " · "
                  plain t(".every_month",
                          day:   @suggestion.day_of_month,
                          count: @suggestion.count,
                          since: I18n.l(@suggestion.first_seen_on, format: :month_year))
                end
                plain " · "
                plain t(".never_invoice")
              end
            end

            div(class: "flex items-center gap-2 flex-none") do
              # Dismiss — POST /money/loans/dismiss with ?key=
              helpers.button_to(
                t(".not_a_loan"),
                helpers.dismiss_money_loan_path,
                method: :post,
                params: { key: @suggestion.key },
                class:  "inline-flex items-center px-3 py-1.5 rounded-lg border border-border text-[12.5px] font-medium text-muted-foreground hover:bg-muted/60 transition-colors",
                form: { data: { turbo: true } }
              )

              # Track — discloses the form below (uses Stimulus if available; fallback summary/details)
              button(type: "button",
                     class: "inline-flex items-center px-3 py-1.5 rounded-lg bg-foreground text-background text-[12.5px] font-medium hover:opacity-90 transition-opacity",
                     data: { action: "click->loan-form#open" }) do
                plain t(".track")
              end
            end
          end

          # Disclosure form (hidden by default; JS toggles it)
          div(id: "loan-suggestion-form-#{@suggestion.key.parameterize}",
              class: "hidden",
              data:  { loan_form_target: "panel" }) do
            render(Campbooks::Money::LoanForm.new(suggestion: @suggestion))
          end
        end
      end

      private

      def helpers
        @helpers_inst ||= begin
          h = super
          h.extend(LoanSuggestionRowHelpers)
          h
        end
      rescue
        super
      end
    end
  end
end
