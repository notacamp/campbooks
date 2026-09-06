# frozen_string_literal: true

module Campbooks
  module Money
    # "Not on a statement" section: invoices/receipts with no bank line on a
    # reconciled statement. Hidden when there are no ready statements.
    class Unbanked < Campbooks::Base
      def initialize(obligations:, evidence:, **attrs)
        @obligations = obligations
        @evidence    = evidence
        @attrs       = attrs
      end

      def view_template
        return unless @evidence.any?

        div(class: @attrs.delete(:class), **@attrs) do
          section_header
          if @obligations.empty?
            p(class: "text-[13.5px] text-muted-foreground") { t(".all_on_statement") }
          else
            @obligations.each { |ob| obligation_row(ob) }
            p(class: "mt-5 text-[12px] text-muted-foreground/70 max-w-[62ch] leading-relaxed") do
              raw safe(t(".evidence_note"))
            end
          end
        end
      end

      private

      def section_header
        div(class: "mb-1 flex flex-wrap items-baseline gap-2") do
          span(class: "text-[11px] font-bold uppercase tracking-widest text-muted-foreground") do
            plain "#{t('.title')}"
            span(class: "font-normal normal-case tracking-normal text-muted-foreground/70") do
              plain " · #{@obligations.size}"
            end
          end
          span(class: "ml-auto text-[12px] text-muted-foreground") { t(".note") }
        end
      end

      def obligation_row(ob)
        div(class: "-mx-4 flex flex-wrap items-start justify-between gap-x-4 gap-y-2 px-4 py-3.5 rounded-xl transition-colors hover:bg-muted/50 sm:-mx-6 sm:px-6") do
          div(class: "min-w-0 flex-1") do
            div(class: "text-[14px] font-semibold text-foreground") { plain ob.counterpart.to_s }
            div(class: "mt-0.5 text-[12.5px] text-muted-foreground") do
              meta_parts(ob).each_with_index do |part, i|
                span(class: "opacity-40 mx-0.5") { plain "·" } if i.positive?
                plain part.to_s
              end
            end
          end
          div(class: "flex shrink-0 flex-col items-end gap-2") do
            amount_cell(ob)
            action_row(ob)
          end
        end
      end

      def meta_parts(ob)
        parts = []
        parts << ob.what.to_s if ob.what.present?
        parts << I18n.l(ob.anchor_on, format: :date) if ob.anchor_on
        stmt_label = ob.statement_label
        parts << t(".not_on_label", label: stmt_label) if stmt_label
        parts << t(".you_sent") if ob.receivable?
        parts
      end

      def amount_cell(ob)
        css = ob.receivable? ? "text-success font-semibold tabular-nums text-[14px]" : "font-semibold tabular-nums text-[14px] text-foreground"
        span(class: css) do
          plain ob.receivable? ? "+#{ob.amount&.format}" : (ob.amount&.format || "-")
        end
      end

      def action_row(ob)
        div(class: "flex flex-wrap gap-1.5") do
          if ob.payable?
            paid_elsewhere_button(ob)
            mark_paid_button(ob)
          elsif ob.receivable?
            send_reminder_button(ob)
            mark_paid_button(ob)
          end
        end
      end

      def mark_paid_button(ob)
        helpers.button_to(t(".mark_paid"),
                          helpers.money_obligation_settle_path(ob.id),
                          class: "inline-flex h-[28px] items-center rounded-lg border border-border px-2.5 text-[12px] font-medium text-muted-foreground hover:bg-muted transition-colors cursor-pointer",
                          method: :post)
      end

      def paid_elsewhere_button(ob)
        helpers.button_to(t(".paid_elsewhere"),
                          helpers.money_obligation_settle_path(ob.id),
                          params: { source: "elsewhere" },
                          class: "inline-flex h-[28px] items-center rounded-lg bg-foreground px-2.5 text-[12px] font-medium text-background hover:opacity-80 transition-opacity cursor-pointer",
                          method: :post)
      end

      def send_reminder_button(ob)
        helpers.button_to(t(".send_reminder"),
                          helpers.money_obligation_chase_path(ob.id),
                          class: "inline-flex h-[28px] items-center rounded-lg bg-foreground px-2.5 text-[12px] font-medium text-background hover:opacity-80 transition-opacity cursor-pointer",
                          method: :post)
      end
    end
  end
end
