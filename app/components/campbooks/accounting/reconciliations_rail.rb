# frozen_string_literal: true

module Campbooks
  module Accounting
    # A compact rail of recent bank statement reconciliations surfaced on the
    # Money page so users can reach reconciliation without going to /accounting.
    #
    # Renders as a section header + a row of statement cards. Each card shows
    # the bank name, period, progress bar, and links to the workbench.
    #
    # @param reconciliations [Array<Reconciliation>]
    class ReconciliationsRail < Campbooks::Base
      def initialize(reconciliations:)
        @reconciliations = reconciliations
      end

      def view_template
        section(class: "mt-10") do
          rail_header
          rail_cards
        end
      end

      private

      def rail_header
        div(class: "flex items-baseline justify-between gap-3 mb-3") do
          div(class: "flex items-baseline gap-2") do
            h2(class: "text-[11.5px] font-semibold uppercase tracking-[0.08em] text-muted-foreground") do
              plain t(".title")
            end
            span(class: "text-[11.5px] text-muted-foreground") { plain @reconciliations.size.to_s }
          end
          a(href: helpers.money_statements_path,
            class: "text-[12.5px] text-muted-foreground hover:text-foreground transition-colors") do
            plain t(".view_all")
          end
        end
      end

      def rail_cards
        div(class: "grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3") do
          @reconciliations.each { |r| statement_card(r) }
        end
      end

      def statement_card(reconciliation)
        a(href: helpers.reconciliation_path(reconciliation),
          class: "group flex flex-col gap-2.5 rounded-2xl border border-border bg-card p-4 hover:border-foreground/20 hover:shadow-sm transition-all") do
          card_header(reconciliation)
          progress_row(reconciliation)
          status_row(reconciliation)
        end
      end

      def card_header(reconciliation)
        div(class: "flex items-start justify-between gap-2") do
          div(class: "min-w-0") do
            div(class: "text-[13.5px] font-semibold text-foreground truncate") do
              plain reconciliation.bank_name.presence || t(".untitled")
            end
            div(class: "mt-0.5 text-[12px] text-muted-foreground") do
              plain reconciliation.period_label.presence || ""
            end
          end
          render(Campbooks::Accounting::ReconciliationStatusBadge.new(status: reconciliation.status))
        end
      end

      def progress_row(reconciliation)
        total    = reconciliation.total_transactions
        resolved = reconciliation.resolved_count
        pct      = total.positive? ? ((resolved.to_f / total) * 100).round : 0

        div do
          div(class: "flex items-baseline justify-between mb-1") do
            span(class: "text-[11px] text-muted-foreground") do
              plain t(".progress", resolved: resolved, total: total)
            end
            span(class: "text-[11px] font-semibold text-foreground") { plain "#{pct}%" }
          end
          div(class: "h-1.5 rounded-full bg-muted overflow-hidden") do
            div(class: "h-full rounded-full bg-success transition-all",
                style: "width:#{pct}%")
          end
        end
      end

      def status_row(reconciliation)
        div(class: "flex items-center justify-between") do
          span(class: "text-[11.5px] text-muted-foreground") do
            if reconciliation.currency.present?
              plain reconciliation.currency
            end
          end
          span(class: "text-[11.5px] text-muted-foreground group-hover:text-foreground transition-colors") do
            plain t(".open")
          end
        end
      end
    end
  end
end
