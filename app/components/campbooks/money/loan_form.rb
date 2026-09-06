# frozen_string_literal: true

module Campbooks
  module Money
    # Inset form panel for tracking a new loan or editing an existing one's terms.
    # Renders inside a disclosure element for show/hide.
    # Uses Phlex-native form elements (helpers.form_with breaks inside Phlex rendering).
    #
    # @param loan       [Loan, nil]
    # @param suggestion [Loans::Spotter::Suggestion, nil]
    class LoanForm < Campbooks::Base
      def initialize(loan: nil, suggestion: nil)
        @loan       = loan
        @suggestion = suggestion
        @editing    = loan.present?
      end

      def view_template
        form_action = @editing ? helpers.money_loan_path(@loan) : helpers.money_loans_path
        method_val  = @editing ? "patch" : "post"

        div(class: "rounded-2xl border border-border bg-muted/40 p-4 mt-3") do
          h4(class: "text-[13.5px] font-semibold mb-0.5") do
            plain @editing ? t(".heading_edit") : t(".heading_new")
          end
          p(class: "text-[12.5px] text-muted-foreground mb-3") do
            plain @editing ? t(".lede_edit") : t(".lede_new")
          end

          form(action: form_action, method: :post,
               data: { turbo: true }) do
            # CSRF
            input(type: "hidden", name: "authenticity_token",
                  value: helpers.form_authenticity_token)
            # Method override for PATCH
            if method_val == "patch"
              input(type: "hidden", name: "_method", value: "patch")
            end
            # Nested key wrapper (controller uses params.require(:loan))
            if source_cp.present?
              input(type: "hidden", name: "loan[source_counterparty]",
                    value: source_cp)
            end

            div(class: "grid grid-cols-1 sm:grid-cols-2 gap-3") do
              field_group("loan[lender]",          "loan_lender",          t(".lender"),
                          prefilled_lender,    hint: prefilled_lender.present? ? t(".hint_from_statements") : nil)
              field_group("loan[instalment]",       "loan_instalment",      t(".instalment"),
                          prefilled_instalment, hint: instalment_hint, css: "tabular-nums")
              field_group("loan[first_instalment_on]", "loan_first_instalment_on", t(".first_instalment_on"),
                          prefilled_first_on, hint: prefilled_first_on.present? ? t(".hint_earliest_found") : nil,
                          type: "date")
              field_group("loan[principal]",        "loan_principal",       t(".principal"),
                          nil, placeholder: t(".placeholder_principal"), css: "tabular-nums")
              field_group("loan[term_months]",      "loan_term_months",     t(".term_months"),
                          prefilled_term, placeholder: t(".placeholder_term"), type: "number")
              field_group("loan[rate_note]",        "loan_rate_note",       t(".rate_note"),
                          @loan&.rate_note, placeholder: t(".placeholder_rate"), hint: t(".hint_rate"))
            end

            div(class: "mt-4 flex justify-end gap-2") do
              button(type: "button",
                     class: "inline-flex items-center px-3 py-1.5 rounded-lg border border-border text-[12.5px] font-medium text-muted-foreground hover:bg-muted/60 transition-colors",
                     data: { action: "click->loan-form#cancel" }) do
                plain t(".cancel")
              end
              input(type: "submit",
                    value: @editing ? t(".save") : t(".track"),
                    class: "inline-flex items-center px-3 py-1.5 rounded-lg bg-foreground text-background text-[12.5px] font-medium hover:opacity-90 transition-opacity cursor-pointer")
            end
          end
        end
      end

      private

      def field_group(name, id, label_text, value, hint: nil, placeholder: nil, type: "text", css: nil)
        div(class: "flex flex-col gap-0.5") do
          label(for: id,
                class: "text-[11.5px] font-semibold text-muted-foreground tracking-wide uppercase") do
            plain label_text
          end
          input(
            type:        type,
            id:          id,
            name:        name,
            value:       value.to_s,
            placeholder: placeholder.to_s,
            class:       class_names(
              "w-full rounded-lg border border-border bg-background px-3 py-2 text-[13px] focus:outline-none focus:ring-2 focus:ring-foreground/30",
              css
            )
          )
          if hint.present?
            span(class: "text-[11.5px] text-muted-foreground mt-0.5") { plain hint }
          end
        end
      end

      def prefilled_lender
        @loan&.lender || @suggestion&.lender_guess
      end

      def source_cp
        @loan&.source_counterparty || @suggestion&.source_counterparty
      end

      def prefilled_instalment
        cents = @loan&.instalment_cents || @suggestion&.instalment_cents
        return nil unless cents

        format_euro(cents)
      end

      def prefilled_first_on
        date = @loan&.first_instalment_on || @suggestion&.first_seen_on
        return nil unless date

        @editing ? date.iso8601 : date.strftime("%-d %b %Y")
      end

      def prefilled_term
        @loan&.term_months
      end

      def instalment_hint
        return nil if @suggestion&.previous_instalment_cents.blank? && @loan&.amount_changed_at.nil?

        if @suggestion&.previous_instalment_cents.present?
          prev = format_euro(@suggestion.previous_instalment_cents)
          t(".hint_was_until", amount: prev)
        elsif (changed = @loan&.amount_changed_at)
          prev = format_euro(changed.previous_amount_cents)
          t(".hint_from_statements_was", amount: prev, month: changed.expected_on.strftime("%b %Y"))
        else
          t(".hint_from_statements")
        end
      end

      def format_euro(cents)
        return "" if cents.nil?

        helpers.number_to_currency(cents / 100.0, unit: "", precision: 2, delimiter: ",", separator: ".")
      end
    end
  end
end
