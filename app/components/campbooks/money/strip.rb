# frozen_string_literal: true

module Campbooks
  module Money
    # The stats row below Scout's read. Hidden when there are no statements.
    # Shows the explained meter, "Need an invoice" (Ember), "Not on a statement",
    # and one compact stat per tracked loan.
    class Strip < Campbooks::Base
      def initialize(read:, **attrs)
        @read  = read
        @attrs = attrs
      end

      def view_template
        return unless @read.any_statements?

        div(class: class_names("flex flex-wrap items-start gap-6 sm:gap-8", @attrs.delete(:class)), **@attrs) do
          focus_stat unless @read.focus_reconciled?
          meter_stat
          no_invoice_stat if @read.needs_invoice_count.positive?
          missing_stat if @read.missing_count.positive?
          @read.loans.each { |loan| render Campbooks::Money::LoanStat.new(loan: loan) }
        end
      end

      private

      # The month to reconcile, when its statement isn't in yet.
      def focus_stat
        pending = @read.pending_statement_count.positive?
        a(href: pending ? "#money_needs" : helpers.new_reconciliation_path, class: "group flex flex-col no-underline transition-opacity hover:opacity-80",
          data: (pending ? {} : { turbo_frame: "_top" })) do
          div(class: "text-[11px] font-semibold uppercase tracking-widest text-muted-foreground") { plain @read.focus_label }
          div(class: "mt-1.5 text-[16px] font-semibold text-muted-foreground") { t(".no_statement_yet") }
          div(class: "mt-0.5 text-[12px] text-foreground underline decoration-border underline-offset-2") do
            t(pending ? ".reconcile_it" : ".add_it")
          end
        end
      end

      def meter_stat
        div(class: "min-w-[160px]") do
          div(class: "text-[11px] font-semibold uppercase tracking-widest text-muted-foreground") do
            plain "#{@read.statement_label}, "
            plain t(".explained")
          end
          div(class: "mt-1.5 text-[16px] font-semibold tabular-nums text-foreground") do
            plain @read.lines_explained.to_s
            span(class: "text-sm font-normal text-muted-foreground") { " / #{@read.lines_total}" }
          end
          div(class: "mt-1.5 h-1.5 w-full overflow-hidden rounded-full bg-muted") do
            div(class: "h-full rounded-full bg-success transition-[width] duration-500", style: "width:#{@read.explained_pct}%")
          end
        end
      end

      def no_invoice_stat
        a(href: "#money_needs", class: "group flex flex-col no-underline transition-opacity hover:opacity-80") do
          div(class: "text-[11px] font-semibold uppercase tracking-widest text-muted-foreground") { t(".needs_invoice") }
          div(class: "mt-1.5 text-[16px] font-semibold tabular-nums text-ember") do
            plain ::Money.new(@read.needs_invoice_cents.to_i, @read.primary_currency).format
          end
          div(class: "mt-0.5 text-[12px] text-muted-foreground") { t(".payments", count: @read.needs_invoice_count) }
        end
      end

      def missing_stat
        a(href: "#money_unbanked", class: "group flex flex-col no-underline transition-opacity hover:opacity-80") do
          div(class: "text-[11px] font-semibold uppercase tracking-widest text-muted-foreground") { t(".not_on_a_statement") }
          div(class: "mt-1.5 text-[16px] font-semibold tabular-nums text-foreground") do
            plain ::Money.new(@read.missing_cents.to_i, @read.primary_currency).format
          end
          div(class: "mt-0.5 text-[12px] text-muted-foreground") { t(".invoices", count: @read.missing_count) }
        end
      end
    end
  end
end
