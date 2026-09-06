# frozen_string_literal: true

module Campbooks
  module Money
    # The stats row below Scout's read. Hidden when there are no statements.
    # Shows: explained progress meter, "Need an invoice" (ember), "Not on a statement".
    class Strip < Campbooks::Base
      def initialize(read:, **attrs)
        @read  = read
        @attrs = attrs
      end

      def view_template
        return unless @read.any_statements?

        div(class: class_names("flex flex-wrap items-start gap-6 sm:gap-8", @attrs.delete(:class)), **@attrs) do
          meter_stat
          if @read.needs_invoice_count.positive?
            no_invoice_stat
          end
          if @read.missing_count.positive?
            missing_stat
          end
        end
      end

      private

      def meter_stat
        div(class: "min-w-[160px]") do
          div(class: "text-[11px] font-semibold uppercase tracking-widest text-muted-foreground") do
            plain "#{@read.statement_label}, "
            plain t(".explained")
          end
          div(class: "mt-1.5 text-[16px] font-semibold tabular-nums text-foreground") do
            plain "#{@read.lines_explained}"
            span(class: "text-muted-foreground font-normal text-sm") { " / #{@read.lines_total}" }
          end
          div(class: "mt-1.5 h-1.5 w-full rounded-full bg-muted overflow-hidden") do
            div(class: "h-full rounded-full bg-success transition-[width] duration-500",
                style: "width:#{@read.explained_pct}%")
          end
        end
      end

      def no_invoice_stat
        a(href: "#money_needs",
          class: "group flex flex-col no-underline hover:opacity-80 transition-opacity") do
          div(class: "text-[11px] font-semibold uppercase tracking-widest text-muted-foreground") do
            plain t(".needs_invoice")
          end
          div(class: "mt-1.5 text-[16px] font-semibold tabular-nums text-ember") do
            amount = ::Money.new(@read.needs_invoice_cents.to_i, @read.primary_currency)
            plain amount.format
          end
          div(class: "text-[12px] text-muted-foreground mt-0.5") do
            plain t(".payments", count: @read.needs_invoice_count)
          end
        end
      end

      def missing_stat
        a(href: "#money_unbanked",
          class: "group flex flex-col no-underline hover:opacity-80 transition-opacity") do
          div(class: "text-[11px] font-semibold uppercase tracking-widest text-muted-foreground") do
            plain t(".not_on_a_statement")
          end
          div(class: "mt-1.5 text-[16px] font-semibold tabular-nums text-foreground") do
            cents = @read.missing_cents.to_i
            plain ::Money.new(cents, @read.primary_currency).format
          end
          div(class: "text-[12px] text-muted-foreground mt-0.5") do
            plain t(".invoices", count: @read.missing_count)
          end
        end
      end
    end
  end
end
