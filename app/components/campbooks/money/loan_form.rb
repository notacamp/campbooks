# frozen_string_literal: true

module Campbooks
  module Money
    # The inset "Track a loan" / "Edit the terms" form. Three things Scout can't
    # read off a statement (amount borrowed, number of instalments, the rate); the
    # rest is prefilled from what it found. Native Phlex form elements (the Rails
    # form helpers write to the wrong buffer inside a component).
    #
    # @param loan       [Loan, nil]                          editing an existing loan
    # @param suggestion [Loans::Spotter::Suggestion, nil]     prefill for a new one
    # @param toggle_id  [String, nil]                         the disclosure checkbox Cancel un-ticks
    class LoanForm < Campbooks::Base
      def initialize(loan: nil, suggestion: nil, toggle_id: nil)
        @loan       = loan
        @suggestion = suggestion
        @toggle_id  = toggle_id
        @editing    = loan.present?
      end

      def view_template
        div(class: "mt-3 rounded-2xl border border-border bg-muted/40 p-4") do
          h4(class: "mb-0.5 text-[13.5px] font-semibold text-foreground") { @editing ? t(".heading_edit") : t(".heading_new") }
          p(class: "mb-3 text-[12.5px] text-muted-foreground") { @editing ? t(".lede_edit") : t(".lede_new") }

          form(action: @editing ? helpers.money_loan_path(@loan) : helpers.money_loans_path, method: :post) do
            input(type: "hidden", name: "authenticity_token", value: helpers.form_authenticity_token)
            input(type: "hidden", name: "_method", value: "patch") if @editing
            input(type: "hidden", name: "loan[source_counterparty]", value: source_counterparty) if source_counterparty.present?

            div(class: "grid grid-cols-1 gap-3 sm:grid-cols-2") do
              field("loan[lender]", t(".lender"), value: prefilled_lender,
                    hint: (t(".hint_from_statements") if prefilled_lender.present? && !@editing))
              field("loan[instalment]", t(".instalment"), value: prefilled_instalment, hint: instalment_hint, css: "tabular-nums")
              field("loan[first_instalment_on]", t(".first_instalment_on"), value: prefilled_first_on, type: "date",
                    hint: (t(".hint_earliest_found") if prefilled_first_on.present? && !@editing))
              field("loan[principal]", t(".principal"), value: prefilled_principal, placeholder: t(".placeholder_principal"),
                    css: "tabular-nums", hint: t(".hint_principal"))
              field("loan[term_months]", t(".term_months"), value: @loan&.term_months, placeholder: t(".placeholder_term"),
                    type: "number", hint: t(".hint_term"))
              field("loan[rate_note]", t(".rate_note"), value: @loan&.rate_note, placeholder: t(".placeholder_rate"), hint: t(".hint_rate"))
            end

            div(class: "mt-4 flex flex-wrap justify-end gap-2") do
              if @toggle_id
                label(for: @toggle_id, class: button_classes(:outline)) { t(".cancel") }
              end
              render(Campbooks::Button.new(variant: :primary, size: :sm, type: "submit")) { @editing ? t(".save") : t(".track") }
            end
          end
        end
      end

      private

      def field(name, label_text, value: nil, hint: nil, placeholder: nil, type: "text", css: nil)
        id = name.to_s.parameterize
        div(class: "flex flex-col gap-0.5") do
          label(for: id, class: "text-[11.5px] font-semibold uppercase tracking-wide text-muted-foreground") { plain label_text }
          input(type: type, id: id, name: name, value: value.to_s, placeholder: placeholder.to_s,
                class: class_names("w-full rounded-lg border border-border bg-background px-3 py-2 text-[13px] text-foreground focus:outline-none focus:ring-2 focus:ring-foreground/30", css))
          span(class: "mt-0.5 text-[11.5px] text-muted-foreground") { plain hint } if hint.present?
        end
      end

      def button_classes(variant)
        class_names(Campbooks::Button::BASE_CLASSES, Campbooks::Button::VARIANT_CLASSES[variant], Campbooks::Button::SIZE_CLASSES[:sm], "cursor-pointer")
      end

      def prefilled_lender
        @loan&.lender || @suggestion&.lender_guess
      end

      def source_counterparty
        @loan&.source_counterparty || @suggestion&.source_counterparty
      end

      def prefilled_instalment
        cents = @loan&.instalment_cents || @suggestion&.instalment_cents
        cents ? plain_amount(cents) : nil
      end

      def prefilled_principal
        @loan ? plain_amount(@loan.principal_cents) : nil
      end

      def prefilled_first_on
        (@loan&.first_instalment_on || @suggestion&.first_seen_on)&.iso8601
      end

      def instalment_hint
        if @suggestion&.previous_instalment_cents.present?
          t(".hint_was_until", amount: plain_amount(@suggestion.previous_instalment_cents))
        elsif (changed = @loan&.amount_changed_at)
          t(".hint_from_statements_was", amount: plain_amount(changed.previous_amount_cents),
                                         month: l(changed.expected_on, format: :month_year))
        elsif @suggestion
          t(".hint_from_statements")
        end
      end

      # "780.00": a plain decimal the controller parses back, no symbol or grouping.
      def plain_amount(cents)
        Kernel.format("%.2f", cents.to_i / 100.0)
      end
    end
  end
end
