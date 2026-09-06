# frozen_string_literal: true

module Campbooks
  module Feed
    # An expense invoice not on a reconciled statement. Shows once Money::Evidence
    # has confirmed that a ready statement covers its date and shows no payment.
    # "Mark paid" settles it; "Open in Money" takes you to the full Money surface.
    class LatePayableCard < Campbooks::Feed::Base
      def view_template
        div(class: "-mx-3 flex items-start gap-3 rounded-2xl px-3 py-3 transition-colors duration-150 hover:bg-muted/50") do
          icon_circle
          div(class: "min-w-0 flex-1") do
            attention_kicker(margin: "mb-1.5")
            div(class: "mt-1 text-sm font-semibold leading-snug text-foreground") { headline }
            div(class: "mt-0.5 text-[12.5px] text-muted-foreground") { eyebrow }
            p(class: "mt-0.5 text-[13px] tabular-nums text-muted-foreground") { amount } if amount
            div(class: "mt-2.5 flex items-center justify-end gap-2") do
              dismiss_button(label: t(".later"), key: "x")
              open_money_button
              act_button(tool: "mark_paid", label: t(".mark_paid"), variant: :primary, key: "p")
            end
          end
        end
      end

      private

      def headline
        t(".headline", name: subject.entity_display_name, reference: reference)
      end

      def eyebrow
        label = item.data["statement_label"].presence
        label ? t(".not_on_statement", label: label) : t(".not_on_a_statement")
      end

      def reference
        subject.invoice_number.present? ? t(".invoice_ref", number: subject.invoice_number) : t(".an_invoice")
      end

      def amount
        cents = item.data["amount_cents"] || subject.amount_cents
        return nil if cents.blank?

        ::Money.new(cents, subject.currency).format
      end

      def open_money_button
        a(href: helpers.money_path,
          class: "inline-flex h-[30px] items-center rounded-lg border border-border px-3 text-[12.5px] font-medium text-foreground no-underline hover:bg-secondary",
          data: { turbo_frame: "_top" }) do
          plain t(".open_money")
        end
      end

      def icon_circle
        span(class: "mt-0.5 flex h-9 w-9 flex-shrink-0 items-center justify-center rounded-full bg-warning/10 text-warning") do
          raw safe(%(
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.9"
                 stroke-linecap="round" stroke-linejoin="round" class="h-[18px] w-[18px]">
              <circle cx="12" cy="12" r="10"/>
              <path d="M12 6v6l4 2"/>
            </svg>
          ).gsub(/\s+/, " ").strip)
        end
      end
    end
  end
end
