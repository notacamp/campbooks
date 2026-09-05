# frozen_string_literal: true

module Campbooks
  module Feed
    # A late receivable on Now: an invoice you sent that hasn't been paid, with the
    # reminder already drafted. "Send reminder" opens the compose Dock prefilled (the
    # Money chase path); "Mark paid" settles it; "Later" hides the card. `subject` is
    # a Document (a revenue invoice). Attention styling — this one wants a decision.
    class LateReceivableCard < Campbooks::Feed::Base
      def view_template
        div(class: "-mx-3 flex items-start gap-3 rounded-2xl px-3 py-3 transition-colors duration-150 hover:bg-muted/50") do
          icon_circle
          div(class: "min-w-0 flex-1") do
            attention_kicker(margin: "mb-1.5")
            div(class: "flex flex-wrap items-center gap-x-1.5 text-[12.5px]") do
              span(class: "font-medium text-foreground") { t(".owed") }
              span(class: "text-muted-foreground/50") { "·" }
              span(class: "font-medium text-warning") { t(".days_late", count: days_late) }
            end
            div(class: "mt-1 text-sm font-semibold leading-snug text-foreground") { headline }
            p(class: "mt-0.5 text-[13px] tabular-nums text-muted-foreground") { amount } if amount
            render Campbooks::ScoutNote.new(message: t(".scout_note"), compact: true, class: "mt-1.5")
            div(class: "mt-2.5 flex items-center justify-end gap-2") do
              dismiss_button(label: t(".later"), key: "x")
              act_button(tool: "mark_paid", label: t(".mark_paid"), variant: :ghost, key: "p")
              send_reminder_button
            end
          end
        end
      end

      private

      def headline
        t(".headline", name: subject.entity_display_name, reference: reference)
      end

      def reference
        subject.invoice_number.present? ? t(".invoice_ref", number: subject.invoice_number) : t(".an_invoice")
      end

      def amount
        cents = item.data["amount_cents"] || subject.amount_cents
        return nil if cents.blank?

        # ::Money — the money-rails class; a bare Money here resolves to Campbooks::Money.
        ::Money.new(cents, subject.currency).format
      end

      def days_late
        item.data["days_late"] || [ (Date.current - subject.due_date.to_date).to_i, 0 ].max
      rescue StandardError
        0
      end

      # A POST that opens the compose Dock with the chase draft (a turbo_stream that
      # updates #compose_dock) — the card stays until the invoice is actually paid.
      def send_reminder_button
        action_form(helpers.money_obligation_chase_path("doc:#{subject.id}")) do
          render Campbooks::Button.new(
            variant: :primary, size: :sm, type: "submit",
            data: feed_action_attrs(key: "s", primary: true).merge(turbo_submits_with: t("components.feed.shared.working"))
          ) do
            plain t(".send_reminder")
            key_chip(key: "s", primary: true)
          end
        end
      end

      def icon_circle
        span(class: "mt-0.5 flex h-9 w-9 flex-shrink-0 items-center justify-center rounded-full bg-warning/10 text-warning") do
          raw safe(%(<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round" class="h-[18px] w-[18px]"><path d="M10.29 3.86 1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"/><path d="M12 9v4M12 17h.01"/></svg>))
        end
      end
    end
  end
end
